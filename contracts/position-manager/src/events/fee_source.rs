use soroban_sdk::contracttype;

/// Which collection routed revenue through the split (§6.11). Opening and
/// closing fees share one split and carry the referral carve-out; borrow has
/// its own LP share and no referral component.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeSource {
    Opening,
    Closing,
    Borrow,
}
