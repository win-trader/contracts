use soroban_sdk::{contractevent, Address};

// Upgrade events live in `interfaces::events` — the
// `TimelockedUpgradeable` trait's default methods emit them, so no
// re-export is needed here.

// NOTE: Deposit/Withdraw/Mint/Redeem events are emitted automatically by OZ's
// stellar_tokens::vault::Vault — see stellar-tokens/src/vault/storage.rs.
// Defining duplicates here would cause the indexer's spec map (keyed by topic
// name) to collide with OZ's specs and mis-parse one of the two events.

/// Vault has paid `amount` to `trader` to settle a position profit. PM is
/// always the caller; the asset moves vault → trader.
/// `new_total_assets` is the post-write absolute vault balance.
#[contractevent(topics = ["pay_profit"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayProfit {
    #[topic]
    pub trader: Address,
    pub amount: i128,
    pub new_total_assets: i128,
}

/// PositionManager has just transferred `amount` USDC into the vault to
/// absorb a trader's loss. The transfer happened off this call (PM does it
/// directly, see ADR-0001); this event lets off-chain indexers keep their
/// tracked total_assets consistent with the vault's on-chain balance.
/// `new_total_assets` is the post-write absolute vault balance.
#[contractevent(topics = ["absorbed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbsorbedCollateral {
    #[topic]
    pub trader: Address,
    pub amount: i128,
    pub new_total_assets: i128,
}

#[contractevent(topics = ["reserve"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reserve {
    pub amount: i128,
    pub new_total: i128,
}

#[contractevent(topics = ["release"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Release {
    pub amount: i128,
    pub new_total: i128,
}

#[contractevent(topics = ["fees"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccrueFees {
    pub amount: i128,
    pub new_total: i128,
}

/// `accrue_fees` could not book the full requested amount: accruing past
/// `total_assets - reserved_usdc` would tag fee claims against capital the
/// vault does not hold, so the accrual is clamped to the available headroom.
/// Monitoring should treat this as a solvency warning — the shortfall
/// (`requested - accrued`) is revenue the dev/staker split silently forfeits.
#[contractevent(topics = ["fees_clamped"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeAccrualClamped {
    pub requested: i128,
    pub accrued: i128,
}

#[contractevent(topics = ["claim"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimFees {
    pub amount: i128,
    pub recipient: Address,
}

#[contractevent(topics = ["net_pnl"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateNetPnl {
    pub pnl: i128,
}

/// Absolute total_assets snapshot, emitted by every LP-facing entrypoint so
/// off-chain indexers can replay state without arithmetic deltas.
#[contractevent(topics = ["total"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TotalAssetsUpdate {
    pub new_total_assets: i128,
}

#[contractevent(topics = ["claim_to"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimFeesTo {
    pub amount: i128,
    pub new_total: i128,
    pub recipient: Address,
}

#[contractevent(topics = ["pause"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pause {
    pub is_paused: bool,
    pub caller: Address,
}

/// Emitted when a deposit/mint records a lockup expiry. Off-chain indexers
/// upsert per-user lockup state from this. The `expires_at` value is the
/// absolute unix timestamp when withdraw/redeem becomes legal.
#[contractevent(topics = ["lockup"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lockup {
    pub user: Address,
    pub expires_at: u64,
}

