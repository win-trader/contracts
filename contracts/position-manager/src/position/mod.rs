pub mod adl;
pub mod entry;
pub mod liquidate;
pub mod mutation;
pub mod trigger;

use soroban_sdk::{panic_with_error, Address, Env};

use crate::errors::PositionManagerError;
use crate::events::{self, CloseReason};
use crate::settle::Settled;

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
