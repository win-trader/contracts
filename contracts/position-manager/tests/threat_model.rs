//! Regression tests for the pre-audit threat-model findings (`THREAT_MODEL.md`).
//!
//! Each test was written to fail against the implementation the threat model
//! examined. The finding each one pins is named in its doc comment.

mod spec_harness;

use spec_harness::*;

use market_governor::MarketGovernorError;
use position_manager::PositionManagerError;
use shared::{defaults, ActionOutcome};
use soroban_sdk::{testutils::Address as _, Symbol};

fn contract_error(code: u32) -> soroban_sdk::Error {
    soroban_sdk::Error::from_contract_error(code)
}

/// T-05 — one timelocked proposal could set `config_timelock_seconds` to one
/// second and make every later proposal effectively instant.
#[test]
fn the_config_timelock_has_a_one_day_floor() {
    let p = Protocol::new();
    let mut shorter = defaults::global_config();
    shorter.config_timelock_seconds = 86_399;
    assert_eq!(
        p.gov().try_propose_global_config(&p.admin, &shorter),
        Err(Ok(contract_error(MarketGovernorError::InvalidConfig as u32)))
    );
    shorter.config_timelock_seconds = 86_400;
    p.gov().propose_global_config(&p.admin, &shorter);
}

/// T-06 — `disable_market` wrote an instance-storage flag for any symbol, so
/// the pause key could grow the entry every call loads.
#[test]
fn only_a_configured_market_can_be_disabled() {
    let p = Protocol::new();
    let c = p.pm();
    assert_eq!(
        c.try_disable_market(&p.admin, &Symbol::new(&p.env, "NOPE")),
        Err(Ok(contract_error(PositionManagerError::MarketNotConfigured as u32)))
    );
    c.disable_market(&p.admin, &p.market);
    assert!(c.is_market_disabled(&p.market));
    c.enable_market(&p.admin, &p.market);
    assert!(!c.is_market_disabled(&p.market));
}

/// T-08 — a `RequiresLiquidation` return published checkpoint events for an
/// accrual it then discarded, so the indexer saw state that never existed.
#[test]
fn a_displaced_action_keeps_the_accrual_it_announced() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();

    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&id, &0);
    p.observe(6, usd(47_600));
    assert_eq!(
        c.settle_close(&p.keeper, &close),
        ActionOutcome::RequiresLiquidation
    );
    assert_eq!(c.get_market(&p.market).last_funding_checkpoint, p.now());
    p.assert_conserved("after a displaced close");
}

/// T-02 — LP settlement priced every registered market, so one market with no
/// usable price, even an empty one, halted every deposit and withdrawal.
#[test]
fn an_empty_market_without_a_price_does_not_block_lp_settlement() {
    let p = Protocol::new();
    // Registered, but the feed has never priced it and nobody trades it.
    p.gov()
        .propose_market_config(&p.admin, &Symbol::new(&p.env, "ETH"), &defaults::market_config());

    let lp = soroban_sdk::Address::generate(&p.env);
    p.mint(&lp, usd(1_000));
    p.router_client().request_deposit(&lp, &usd(1_000));
    p.observe(defaults::LP_REQUEST_DELAY_LOCAL, FILL);
    assert_eq!(
        p.router_client().resolve_next(&soroban_sdk::Address::generate(&p.env)).status,
        shared::SettlementStatus::Settled
    );
    p.assert_conserved("after settling past an unpriced empty market");
}

/// T-02 — a queue head that cannot resolve (here: a live market whose price
/// went stale) blocked every LP request behind it, with no way past. The pause
/// key can now refund it in full and move on, but only after a day's grace.
#[test]
fn the_pause_key_can_refund_a_stuck_queue_head_after_a_grace_period() {
    use request_router::RequestRouterError;
    let p = Protocol::new();
    p.open_position();
    let lp = soroban_sdk::Address::generate(&p.env);
    p.mint(&lp, usd(1_000));
    let id = p.router_client().request_deposit(&lp, &usd(1_000));

    // The live market's feed stops publishing past max_price_age.
    p.wait(defaults::LP_REQUEST_DELAY_LOCAL + defaults::MAX_PRICE_AGE_SECONDS + 1);
    let executor = soroban_sdk::Address::generate(&p.env);
    assert!(p.router_client().try_resolve_next(&executor).is_err(), "the head is stuck");

    let outsider = soroban_sdk::Address::generate(&p.env);
    assert_eq!(
        p.router_client().try_skip_head(&outsider),
        Err(Ok(contract_error(RequestRouterError::Unauthorized as u32)))
    );
    assert_eq!(
        p.router_client().try_skip_head(&p.admin),
        Err(Ok(contract_error(RequestRouterError::TooEarly as u32)))
    );

    p.wait(86_400);
    p.router_client().skip_head(&p.admin);
    assert_eq!(p.cash(&lp), usd(1_000), "the whole escrow comes back, no reward taken");
    assert_eq!(p.router_client().next_request_to_resolve(), id + 1);
    assert_eq!(
        p.router_client().get_request(&id).status,
        shared::LpRequestStatus::Failed
    );
    // The conservation check prices the book, so the feed has to be live again.
    p.publish(FILL);
    p.assert_conserved("after skipping a stuck head");
}

/// T-13 — LP settlement costs about 12M instructions per registered market,
/// and nothing bounded the registry.
#[test]
fn the_market_registry_is_capped_at_sixteen() {
    let p = Protocol::new();
    let mut wide = defaults::global_config();
    wide.max_active_markets = 17;
    assert_eq!(
        p.gov().try_propose_global_config(&p.admin, &wide),
        Err(Ok(contract_error(MarketGovernorError::InvalidConfig as u32)))
    );
    wide.max_active_markets = 16;
    p.gov().propose_global_config(&p.admin, &wide);
}

/// T-22 — the LP resolve reward was read at resolution, so a reward raised
/// after a request was queued charged that request the new rate.
#[test]
fn a_queued_lp_request_pays_the_reward_in_force_when_it_was_made() {
    let p = Protocol::new();
    let lp = soroban_sdk::Address::generate(&p.env);
    p.mint(&lp, usd(1_000));
    let id = p.router_client().request_deposit(&lp, &usd(1_000));
    let queued = defaults::KEEPER_REWARD;
    assert_eq!(p.router_client().get_request(&id).reward, queued);

    let mut raised = defaults::global_config();
    raised.keeper_rewards.lp_resolve = queued * 4;
    p.gov().propose_global_config(&p.admin, &raised);
    p.observe(defaults::CONFIG_TIMELOCK_SECONDS, FILL);
    p.gov().apply_global_config(&p.keeper);
    assert_eq!(p.vault_client().lp_resolve_reward(), queued * 4);

    let settled = p.router_client().resolve_next(&soroban_sdk::Address::generate(&p.env));
    assert_eq!(settled.status, shared::SettlementStatus::Settled);
    assert_eq!(settled.reward, queued, "the queued request keeps its original reward");
}

/// T-04 — `set_lp_config` is instant, and a withdrawal-utilization gate of 0
/// froze every LP withdrawal without notice.
#[test]
fn the_admin_cannot_close_the_withdrawal_gate() {
    use vault::VaultError;
    let p = Protocol::new();
    let mut config = p.vault_client().get_lp_config();
    config.max_withdraw_utilization_bps = 0;
    assert_eq!(
        p.vault_client().try_set_lp_config(&p.admin, &config),
        Err(Ok(contract_error(VaultError::InvalidConfig as u32)))
    );
    config.max_withdraw_utilization_bps = 4_999;
    assert!(p.vault_client().try_set_lp_config(&p.admin, &config).is_err());
    config.max_withdraw_utilization_bps = 5_000;
    p.vault_client().set_lp_config(&p.admin, &config);
}

/// R-1 — the one-shot wirings left no event, so the audit trail could not show
/// who connected the PositionManager to its vault, or the vault to its router.
#[test]
fn one_shot_wirings_are_evented() {
    use soroban_sdk::testutils::Events as _;
    use soroban_sdk::{TryFromVal, Val, Vec};
    let p = Protocol::new();
    let wired = |source: &soroban_sdk::Address| -> Vec<Val> {
        let topic = Symbol::new(&p.env, "wired");
        p.env
            .events()
            .all()
            .iter()
            .find(|(src, topics, _)| {
                src == source
                    && topics.first().map(|t| Symbol::try_from_val(&p.env, &t) == Ok(topic.clone())).unwrap_or(false)
            })
            .map(|(_, _, data)| Vec::<Val>::try_from_val(&p.env, &data).unwrap())
            .expect("a wired event")
    };

    let pm = p.env.register(
        position_manager::PositionManagerContract,
        (
            p.config_manager.clone(),
            soroban_sdk::Address::generate(&p.env),
            p.feed.clone(),
            defaults::global_config(),
        ),
    );
    position_manager::PositionManagerContractClient::new(&p.env, &pm).set_vault(&p.admin, &p.vault);
    let data = wired(&pm);
    assert_eq!(Symbol::try_from_val(&p.env, &data.get(0).unwrap()), Ok(Symbol::new(&p.env, "vault")));
    assert_eq!(soroban_sdk::Address::try_from_val(&p.env, &data.get(2).unwrap()), Ok(p.admin.clone()));

    let vault = p.env.register(
        vault::VaultContract,
        (p.token.clone(), p.config_manager.clone(), pm.clone(), p.vault_client().get_lp_config()),
    );
    vault::VaultContractClient::new(&p.env, &vault).set_request_router(&p.admin, &p.router);
    let data = wired(&vault);
    assert_eq!(
        Symbol::try_from_val(&p.env, &data.get(0).unwrap()),
        Ok(Symbol::new(&p.env, "request_router"))
    );
}
