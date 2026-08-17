use soroban_sdk::contracttype;

/// Which collection routed revenue through the split (§13).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeSource {
    Closing,
    Borrow,
}
