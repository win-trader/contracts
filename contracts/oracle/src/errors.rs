use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OracleError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    NoPriceSet = 4,
    /// `upgrade` rejected — no `propose_upgrade` was made before commit.
    NoPendingUpgrade = 5,
    /// `upgrade` rejected — timelock has not elapsed yet.
    UpgradeTimelockNotElapsed = 6,
    /// `upgrade` rejected — `new_wasm_hash` does not match the proposed hash.
    UpgradeHashMismatch = 7,
}
