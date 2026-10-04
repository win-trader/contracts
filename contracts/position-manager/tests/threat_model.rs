//! Regression tests for the pre-audit threat-model findings (`THREAT_MODEL.md`).
//!
//! Each test was written to fail against the implementation the threat model
//! examined. The finding each one pins is named in its doc comment.

mod spec_harness;

use spec_harness::*;

use market_governor::MarketGovernorError;
use position_manager::PositionManagerError;
use shared::{defaults, ActionOutcome};
use soroban_sdk::Symbol;

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
