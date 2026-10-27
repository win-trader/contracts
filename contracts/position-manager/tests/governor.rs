//! The governor split: the PositionManager accepts configuration only from its
//! MarketGovernor, and re-checks everything it installs.

mod spec_harness;

use spec_harness::*;

use market_governor::MarketGovernorError;
use position_manager::PositionManagerError;
use shared::constants::ROLE_ORACLE;
use shared::defaults;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Map, Symbol, TryFromVal, Val};

fn contract_error(code: u32) -> soroban_sdk::Error {
    soroban_sdk::Error::from_contract_error(code)
}

fn grant(p: &Protocol, role: &str) -> Address {
    let who = Address::generate(&p.env);
    config_manager::ConfigManagerClient::new(&p.env, &p.config_manager).grant_role(
        &p.admin,
        &Symbol::new(&p.env, role),
        &who,
    );
    who
}

/// Only the governor may install configuration; even the ADMIN key must go
/// through the governor's timelock.
#[test]
fn the_position_manager_refuses_configuration_from_anyone_but_its_governor() {
    let p = Protocol::new();
    let c = p.pm();
    let invalid_caller = Err(Ok(contract_error(PositionManagerError::InvalidCaller as u32)));
    assert_eq!(
        c.try_install_global_config(&p.admin, &p.admin, &defaults::global_config()),
        invalid_caller
    );
    assert_eq!(
        c.try_install_market_config(&p.admin, &p.admin, &p.market, &defaults::market_config()),
        invalid_caller
    );
    assert_eq!(c.try_install_price_feed(&p.admin, &p.admin, &p.feed), invalid_caller);
    assert_eq!(c.try_deregister_market(&p.admin, &p.admin, &p.market), invalid_caller);
    assert_eq!(c.governor(), p.governor);
    assert_eq!(p.gov().position_manager(), p.pm);
}

/// A compromised governor still cannot install a config that breaks the
/// position manager's own invariants or its bounds on the market set.
#[test]
fn the_position_manager_rechecks_what_the_governor_installs() {
    let p = Protocol::new();
    let c = p.pm();

    let mut broken = defaults::global_config();
    broken.min_collateral = 0;
    assert_eq!(
        c.try_install_global_config(&p.governor, &p.admin, &broken),
        Err(Ok(contract_error(PositionManagerError::InvalidConfig as u32)))
    );

    let mut broken_market = defaults::market_config();
    broken_market.maintenance_margin_bps = broken_market.initial_margin_bps + 1;
    assert_eq!(
        c.try_install_market_config(&p.governor, &p.admin, &p.market, &broken_market),
        Err(Ok(contract_error(PositionManagerError::InvalidConfig as u32)))
    );

    // The registry stays bounded (THREAT_MODEL T-13) whoever asks.
    let mut tight = defaults::global_config();
    tight.max_active_markets = 1;
    c.install_global_config(&p.governor, &p.admin, &tight);
    assert_eq!(
        c.try_install_market_config(
            &p.governor,
            &p.admin,
            &Symbol::new(&p.env, "ETH"),
            &defaults::market_config()
        ),
        Err(Ok(contract_error(PositionManagerError::MarketLimitExceeded as u32)))
    );
}

/// The governor rejects an invalid proposal up front instead of letting it
/// sit out the timelock and fail at apply.
#[test]
fn the_governor_rejects_an_invalid_proposal_at_proposal_time() {
    let p = Protocol::new();
    let mut broken = defaults::market_config();
    broken.order_execution_delay_seconds = 0;
    assert_eq!(
        p.gov().try_propose_market_config(&p.admin, &p.market, &broken),
        Err(Ok(contract_error(MarketGovernorError::InvalidConfig as u32)))
    );
}

/// THREAT_MODEL T-07 — revoking a compromised ORACLE key must be enough:
/// ADMIN can cancel the feed proposal it left behind.
#[test]
fn admin_can_cancel_a_price_feed_proposal() {
    let p = Protocol::new();
    let oracle = grant(&p, ROLE_ORACLE);
    let replacement = p.env.register(mock_oracle::MockOracle, ());
    p.gov().propose_price_feed(&oracle, &replacement);

    let outsider = Address::generate(&p.env);
    assert_eq!(
        p.gov().try_cancel_price_feed(&outsider),
        Err(Ok(contract_error(MarketGovernorError::Unauthorized as u32)))
    );
    p.gov().cancel_price_feed(&p.admin);
    p.observe(defaults::CONFIG_TIMELOCK_SECONDS, FILL);
    assert!(p.gov().try_apply_price_feed(&p.keeper).is_err(), "nothing left to apply");
    assert_eq!(p.pm().price_feed(), p.feed);
}

/// The installed-config event names whoever triggered the change, not the
/// governor contract that relayed it.
#[test]
fn an_installed_config_is_attributed_to_its_actor() {
    let p = Protocol::new();
    let mut lower = defaults::global_config();
    lower.risk_capacity_limit_bps -= 1;
    // Lowering capacity is conservative, so it installs immediately.
    p.gov().propose_global_config(&p.admin, &lower);

    let wanted = Symbol::new(&p.env, "cfgglobal");
    let actors: Vec<Address> = p
        .env
        .events()
        .all()
        .iter()
        .filter(|(source, topics, _)| {
            *source == p.pm
                && topics
                    .first()
                    .map(|t| Symbol::try_from_val(&p.env, &t) == Ok(wanted.clone()))
                    .unwrap_or(false)
        })
        .map(|(_, _, data)| {
            let m = Map::<Symbol, Val>::try_from_val(&p.env, &data).unwrap();
            let h = Map::<Symbol, Val>::try_from_val(
                &p.env,
                &m.get(Symbol::new(&p.env, "header")).unwrap(),
            )
            .unwrap();
            Address::try_from_val(&p.env, &h.get(Symbol::new(&p.env, "actor")).unwrap()).unwrap()
        })
        .collect();
    assert_eq!(actors, vec![p.admin.clone()]);
    assert_eq!(p.pm().global_config(), lower);
}
