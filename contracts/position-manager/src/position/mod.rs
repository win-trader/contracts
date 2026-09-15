//! The user-facing operations of §7, one module per lifecycle stage.

pub mod adl;
pub mod entry;
pub mod liquidate;
pub mod mutation;
pub mod trigger;

use soroban_sdk::{panic_with_error, Address, Env};

use crate::errors::PositionManagerError;
use crate::events::{self, CloseReason};
use crate::settle::Settled;

/// Emit the result of a settlement that removes the complete position.
///
/// Every terminal path passes the position's full remaining size, so a
/// `Partial` result here would mean the settlement disagreed with its own
/// input — an invariant break, not a business outcome.
pub(crate) fn emit_terminal(
    env: &Env,
    actor: &Address,
    settled: &Settled,
    reason: CloseReason,
) {
    match settled {
        Settled::Closed(header, tail) => events::emit_closed(env, actor, header, tail, reason),
        Settled::Partial(_) => {
            panic_with_error!(env, PositionManagerError::InvariantViolation)
        }
    }
}
