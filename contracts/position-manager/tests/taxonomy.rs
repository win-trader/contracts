//! §12.5 — the error taxonomy, checked as a property rather than trusted.
//!
//! The rule the ranges exist for is that a raw code is globally unambiguous.
//! Before this, every contract numbered from `1`, so code `9` meant
//! `SlippageExceeded` in the position manager, `ArithmeticError` in the
//! vault, and `UpgradeTimelockNotElapsed` in the request router — three
//! conditions with three different remedies behind one number, which is how
//! the app came to report oracle outages as slippage.

use config_manager::ConfigManagerError as Cm;
use position_manager::PositionManagerError as Pm;
use request_router::RequestRouterError as Rr;
use vault::VaultError as V;

/// Every code this build can emit, by owning contract and assigned range.
fn ranges() -> [(&'static str, u32, u32, Vec<u32>); 4] {
    [
        (
            "position-manager",
            1,
            99,
            vec![
                Pm::Unauthorized as u32,
                Pm::InvalidCaller as u32,
                Pm::PositionNotFound as u32,
                Pm::MarketNotConfigured as u32,
                Pm::ActionNotFound as u32,
                Pm::TriggerNotAttached as u32,
                Pm::NoPendingConfig as u32,
                Pm::UpgradeNoPending as u32,
                Pm::NotInitialized as u32,
                Pm::AlreadyInitialized as u32,
                Pm::Paused as u32,
                Pm::MarketDisabled as u32,
                Pm::RiskStateBlocked as u32,
                Pm::PositionHealthy as u32,
                Pm::TooEarly as u32,
                Pm::MutationPending as u32,
                Pm::MarketNotEmpty as u32,
                Pm::StateVersionMismatch as u32,
                Pm::ConfigTimelockNotElapsed as u32,
                Pm::UpgradeTimelockNotElapsed as u32,
                Pm::InsufficientCollateral as u32,
                Pm::MarketLimitExceeded as u32,
                Pm::InvalidAmount as u32,
                Pm::InvalidConfig as u32,
                Pm::InvalidOrder as u32,
                Pm::WrongActionKind as u32,
                Pm::UpgradeHashMismatch as u32,
                Pm::PriceUnavailable as u32,
                Pm::StalePrice as u32,
                Pm::InvariantViolation as u32,
                Pm::ArithmeticError as u32,
            ],
        ),
        (
            "vault",
            100,
            199,
            vec![
                V::Unauthorized as u32,
                V::InvalidCaller as u32,
                V::NotInitialized as u32,
                V::AlreadyInitialized as u32,
                V::Paused as u32,
                V::InsufficientCash as u32,
                V::InvalidAmount as u32,
                V::InvalidConfig as u32,
                V::UpgradeNoPending as u32,
                V::UpgradeTimelockNotElapsed as u32,
                V::UpgradeHashMismatch as u32,
                V::ArithmeticError as u32,
            ],
        ),
        (
            "config-manager",
            300,
            399,
            vec![
                Cm::Unauthorized as u32,
                Cm::NotPendingAdmin as u32,
                Cm::NoPendingAdmin as u32,
                Cm::NoPendingUpgrade as u32,
                Cm::NotInitialized as u32,
                Cm::AlreadyInitialized as u32,
                Cm::UpgradeTimelockNotElapsed as u32,
                Cm::AdminProposalExpired as u32,
                Cm::InvalidAdminProposal as u32,
                Cm::UpgradeTimelockTooShort as u32,
                Cm::UpgradeTimelockTooLong as u32,
                Cm::UpgradeHashMismatch as u32,
            ],
        ),
        (
            "request-router",
            400,
            499,
            vec![
                Rr::Unauthorized as u32,
                Rr::InvalidRequest as u32,
                Rr::LpActionBlocked as u32,
                Rr::InvalidAmount as u32,
                Rr::UpgradeNoPending as u32,
                Rr::UpgradeTimelockNotElapsed as u32,
                Rr::UpgradeHashMismatch as u32,
            ],
        ),
    ]
}

#[test]
fn every_code_sits_inside_its_owner_range() {
    for (name, low, high, codes) in ranges() {
        for code in codes {
            assert!(
                code >= low && code <= high,
                "{name} code {code} is outside {low}-{high}"
            );
        }
    }
}

/// The property that makes a raw code globally unambiguous.
#[test]
fn no_code_is_owned_by_two_contracts() {
    let mut seen: Vec<(u32, &'static str)> = Vec::new();
    for (name, _, _, codes) in ranges() {
        for code in codes {
            if let Some((_, other)) = seen.iter().find(|(c, _)| *c == code) {
                panic!("code {code} is claimed by both {other} and {name}");
            }
            seen.push((code, name));
        }
    }
}

/// Within a contract, a code is assigned once and never reused.
#[test]
fn no_contract_assigns_one_code_twice() {
    for (name, _, _, codes) in ranges() {
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(before, sorted.len(), "{name} reuses a code");
    }
}

/// §12.5 — the oracle router's `200–299` is left vacant rather than
/// recycled. A code from a decommissioned deployment must not come back
/// looking like a live one.
#[test]
fn the_deleted_oracle_routers_range_stays_vacant() {
    for (name, _, _, codes) in ranges() {
        for code in codes {
            assert!(
                !(200..=299).contains(&code),
                "{name} code {code} reuses the deleted oracle router's range"
            );
        }
    }
}
