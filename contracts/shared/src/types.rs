use soroban_sdk::{contracttype, Address, BytesN, Symbol};

/// §5.5 — one open position.
///
/// There is no separately authoritative entry price: size and base exposure
/// together preserve the complete price exposure, including across multiple
/// increases. There is also no execution budget, keeper reserve allocation,
/// accrued-fee counter, cached PnL, cached effective collateral, or cached
/// health value — all of those are derived (§5.13).
#[contracttype]
#[derive(Clone, Debug)]
pub struct Position {
    pub id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub is_long: bool,
    /// USD notional at `PRICE_PRECISION`.
    pub size: i128,
    /// Asset units at `PRICE_PRECISION`.
    pub base_exposure: i128,
    /// Trader-owned collateral recorded in contract state (the doc's
    /// "stored collateral"). Effective collateral — stored collateral after
    /// pending fees and funding credits — is always derived, never stored.
    pub stored_collateral: i128,
    /// Fixed gross capacity assigned when risk opens.
    pub risk_units: i128,
    /// Rounded cumulative borrow value at the active window's baseline.
    pub borrow_debt: i128,
    /// §3.3.2 fixed monetary minimum for the **active borrow window**,
    /// quoted when the window opens. Not an index delta: the floor is
    /// per-window, and nothing of it carries into the next one (§3.3.3).
    pub stored_minimum_borrow_fee: i128,
    pub funding_paid_to_receivers_debt: i128,
    pub funding_paid_to_lps_debt: i128,
    pub funding_received_debt: i128,
    /// Time the first exposure was successfully created.
    pub opened_at: u64,
    /// Start of the current minimum-position-lifetime restriction. A size
    /// increase moves it; adding collateral does not (§7.7).
    pub last_size_increase_at: u64,
    /// §8.4 — the one pending voluntary increase, decrease, or close for
    /// this position. An O(1) reverse reference: creation fails while it is
    /// occupied, so conflicting mutations cannot be committed against the
    /// same pre-action state. Liquidation and ADL may invalidate it as
    /// forced safety actions.
    pub pending_mutation_action_id: Option<u64>,
    /// Attached take-profit instruction. Take-profit and stop-loss may
    /// coexist because they are opposite exit conditions; both are position
    /// instructions, not prepaid keeper budgets.
    pub take_profit: Trigger,
    /// Attached stop-loss instruction.
    pub stop_loss: Trigger,
}

/// §5.5's "optional trigger", as an enum rather than `Option`.
///
/// `#[contracttype]` generates only a *fallible* `TryFrom<&T> for ScVal`,
/// while `Option<T>`'s XDR conversion needs an infallible `Into<ScVal>` for
/// its inner type. So `Option<TriggerInstruction>` compiles for wasm and
/// fails the moment `testutils` is on — a trap worth closing in the type
/// rather than rediscovering. A two-variant enum carries exactly the same
/// information and converts cleanly. (`Option<u64>` is fine and stays:
/// `u64` does have the infallible conversion.)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Trigger {
    None,
    Attached(TriggerInstruction),
}

impl Trigger {
    pub fn instruction(&self) -> Option<&TriggerInstruction> {
        match self {
            Trigger::Attached(i) => Some(i),
            Trigger::None => None,
        }
    }

    pub fn is_attached(&self) -> bool {
        matches!(self, Trigger::Attached(_))
    }

    /// The attached trigger price, or `0` for none — the sentinel the
    /// current event set still reports.
    pub fn price(&self) -> i128 {
        match self {
            Trigger::Attached(i) => i.trigger_price,
            Trigger::None => 0,
        }
    }
}

/// §5.5 — the minimum an attached trigger needs to execute safely.
///
/// `commit_observed_at` is the oracle cursor at the moment the instruction
/// was attached or replaced; an execution observation must be strictly
/// newer (§7.0), which is what stops a trigger from closing a position on
/// the same observation that armed it.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggerInstruction {
    pub trigger_price: i128,
    /// Exit price bound; `0` disables it. A crossed trigger whose bound
    /// fails leaves the instruction **attached** and pays nothing (§8.7).
    pub acceptable_price: i128,
    pub committed_at: u64,
    pub execute_after: u64,
    pub commit_observed_at: u64,
}

// ---------------------------------------------------------------------------
// §5.6 Pending-action state.
//
// Trader-requested price-sensitive mutations are one unified record. Only
// pending actions are stored as executable objects; terminal outcomes remove
// the record and survive only as their emitted result (§12.6).
// ---------------------------------------------------------------------------

/// Which payload a `PendingAction` carries.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionKind {
    MarketOpen,
    LimitOpen,
    Increase,
    Decrease,
    Close,
}

/// §5.6 — the `kind`-selected immutable payload.
///
/// The payload freezes **every trader-controlled economic input**.
/// Settlement may read current vault state and current position state, but
/// it must never replace a committed size, trigger, acceptable price,
/// direction, or collateral amount with new caller input.
#[contracttype]
#[derive(Clone, Debug)]
pub enum ActionPayload {
    MarketOpen(OpenPayload),
    /// Adds a trigger and its frozen direction to the open payload.
    LimitOpen(OpenPayload, TriggerCondition),
    Increase(IncreasePayload),
    Decrease(DecreasePayload),
    Close(ClosePayload),
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct OpenPayload {
    pub is_long: bool,
    pub size: i128,
    pub submitted_collateral: i128,
    pub acceptable_price: i128,
    pub expires_at: u64,
    pub take_profit: i128,
    pub stop_loss: i128,
}

/// The limit-entry trigger, with `trigger_above` frozen at creation against
/// the authenticated commit price rather than re-derived at settlement.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriggerCondition {
    pub trigger_price: i128,
    pub trigger_above: bool,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct IncreasePayload {
    pub position_id: u64,
    pub size_added: i128,
    pub collateral_added: i128,
    pub acceptable_price: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct DecreasePayload {
    pub position_id: u64,
    pub size_removed: i128,
    pub acceptable_price: i128,
}

/// A close always targets the complete remaining exposure at settlement, so
/// it stores no size (§7.10).
#[contracttype]
#[derive(Clone, Debug)]
pub struct ClosePayload {
    pub position_id: u64,
    pub acceptable_price: i128,
}

/// §5.6 — a committed, not-yet-settled trader action.
///
/// `execute_after` is frozen at creation from the market's
/// `order_execution_delay_seconds`, so a later configuration change does not
/// alter an existing commitment (§10.3.3). `commit_observed_at` is the
/// qualifying oracle cursor recorded at creation; an execution observation
/// must be **strictly** newer (§7.0 — equality fails).
///
/// An action ID is consumed permanently when the record is removed. A later
/// call with that ID fails as nonexistent and cannot replay its transfer or
/// its keeper payment (§8.13).
#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingAction {
    pub action_id: u64,
    pub owner: Address,
    pub market_id: Symbol,
    pub kind: ActionKind,
    pub created_at: u64,
    pub execute_after: u64,
    pub commit_observed_at: u64,
    /// §5.7 — collateral transferred at creation and held inside the vault.
    /// The aggregate lives in `Ledger::action_escrow_total`.
    pub escrowed_collateral: i128,
    pub payload: ActionPayload,
}

impl ActionPayload {
    /// The open payload of a market or limit entry.
    pub fn open(&self) -> Option<&OpenPayload> {
        match self {
            ActionPayload::MarketOpen(open) | ActionPayload::LimitOpen(open, _) => Some(open),
            _ => None,
        }
    }

    /// The position an increase, decrease, or close targets.
    pub fn position_id(&self) -> Option<u64> {
        match self {
            ActionPayload::Increase(p) => Some(p.position_id),
            ActionPayload::Decrease(p) => Some(p.position_id),
            ActionPayload::Close(p) => Some(p.position_id),
            _ => None,
        }
    }

    /// The trader's committed price bound. `0` disables it everywhere.
    pub fn acceptable_price(&self) -> i128 {
        match self {
            ActionPayload::MarketOpen(o) | ActionPayload::LimitOpen(o, _) => o.acceptable_price,
            ActionPayload::Increase(p) => p.acceptable_price,
            ActionPayload::Decrease(p) => p.acceptable_price,
            ActionPayload::Close(p) => p.acceptable_price,
        }
    }
}

/// §7.0 — how one settlement attempt ended.
///
/// The first five are **terminal**: the action record is removed. The last
/// three change no state and pay nothing, and they *return* rather than
/// panicking — a panic reverts, hands the trader a free retry, and pays no
/// keeper.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionOutcome {
    Executed,
    Failed,
    Cancelled,
    Expired,
    Superseded,
    /// The delay has not elapsed, or no strictly newer observation exists.
    NotReady,
    /// A limit trigger the observation has not crossed. Does not consume
    /// the order (§7.4).
    Pending,
    /// The position is liquidatable: the only non-terminal safety exception
    /// for a position mutation (§7.0).
    RequiresLiquidation,
}

/// §8.9 — why an action terminated without executing. These are expected
/// business outcomes, not errors: they complete successfully and are
/// recorded, and they never appear as an error code (§12.5).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureReason {
    PriceBoundExceeded,
    CapacityExceeded,
    ExposureCapExceeded,
    SideRestricted,
    InsufficientCollateral,
    /// §7.9 — cash LP equity could not cover the payable profit a surviving
    /// decrease would credit. Distinct from `InsufficientCollateral`, which
    /// is about the position; this is about the vault.
    ///
    /// A survivor may not realize profit the vault could not pay: unlike a
    /// terminal settlement it has no result in which to report the
    /// shortfall, and its closing fee would be computed from profit that
    /// was never credited.
    UnpayableProfit,
    PositionGone,
    MarketPaused,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiskState {
    Normal,
    Warning,
    Adl,
    HardCap,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MarketSide {
    pub size_open_interest: i128,
    pub base_exposure: i128,
    pub stored_collateral_total: i128,
    pub risk_units: i128,
    pub risk_state: RiskState,
    /// §6.5 — `INDEX_PRECISION` unless the side is latched in `HardCap`.
    /// A **stored snapshot**, taken on entry and on each §6.16 band
    /// re-latch, never recomputed live: deriving it per settlement makes
    /// payouts order-dependent, because each settlement moves both LP
    /// equity and the side's aggregate PnL, so the next position out is
    /// measured against a shrunken denominator.
    pub hard_cap_payout_factor: i128,
    /// Side positive PnL the current payout factor was measured against;
    /// `0` unless latched in `HardCap`.
    pub hard_cap_reference_pnl: i128,
}

impl MarketSide {
    pub fn new() -> Self {
        MarketSide {
            size_open_interest: 0,
            base_exposure: 0,
            stored_collateral_total: 0,
            risk_units: 0,
            risk_state: RiskState::Normal,
            hard_cap_payout_factor: crate::constants::INDEX_PRECISION,
            hard_cap_reference_pnl: 0,
        }
    }
}

impl Default for MarketSide {
    fn default() -> Self {
        Self::new()
    }
}

/// Which side currently pays funding (§8.1: the side the blended integral
/// skew points at — under the EMA this can be the *lighter* side for a
/// while after the book flips). `None` when the blend is exactly zero.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayerSide {
    None,
    Long,
    Short,
}

/// §5.3 — one market's trading, funding, fee, risk, and execution policy.
///
/// Fee rates are independent of market skew. Liquidation and ADL rewards are
/// global fixed amounts (§5.10), not market percentages.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketConfig {
    /// §3.1 opening fee on added size.
    pub open_fee_bps: u32,
    /// §3.2.1 closing size component.
    pub close_size_fee_bps: u32,
    /// §3.2.2 closing PnL component. There is deliberately no ordering
    /// relationship between this and `close_size_fee_bps`: the charged
    /// target is their maximum and the §3.2.3 profit caps bound it.
    pub close_pnl_fee_bps: u32,
    pub max_funding_rate_bps_day: i128,
    /// §3.4 weight of the instantaneous skew in the funding blend, in bps;
    /// the rest is the half-life EMA. `BPS` reproduces pure instant skew.
    pub instant_weight_bps: u32,
    /// §2.7 share of position size converted into risk units. §5.3 permits
    /// a change only while both sides have zero open interest and zero risk
    /// units — otherwise every live position's canonical risk units would
    /// diverge from the value derived from its size, and there is no
    /// bounded way to rewrite them.
    pub market_risk_factor_bps: u32,
    /// Margin required to open or add risk (§6.6). Divides max leverage:
    /// a position may not be created closer to liquidation than this.
    pub initial_margin_bps: u32,
    /// Margin below which the position is liquidatable (§6.15). Must not
    /// exceed `initial_margin_bps`; the gap is the entry buffer.
    pub maintenance_margin_bps: u32,
    pub recovery_pnl_factor_bps: u32,
    pub warning_pnl_factor_bps: u32,
    pub adl_pnl_factor_bps: u32,
    pub hard_cap_pnl_factor_bps: u32,
    pub max_long_size_open_interest: i128,
    pub max_short_size_open_interest: i128,
    pub max_long_base_exposure: i128,
    pub max_short_base_exposure: i128,
    /// §5.3 minimum delay for trader-requested actions on this market,
    /// validated from `1` through `30`. Frozen into each action's
    /// `execute_after` at creation (§10.3.3), so a later change never
    /// alters an existing commitment.
    pub order_execution_delay_seconds: u64,
}

/// §5.10 — the fixed keeper rewards. Eleven **independent** fields, even
/// though every initial value is the same `2_500_000` ($0.25): the
/// configuration stores no generic execution reward and no user override,
/// and every settlement kind maps to exactly one field (§6.12).
///
/// §10.3.1 requires `every reward <= min_collateral` and
/// `min_collateral > liquidation`. That blanket bound is load-bearing, not
/// cosmetic: without it, raising `close` above `liquidation` would create
/// positions that are neither liquidatable — their effective collateral is
/// above the liquidation threshold — nor closeable, because paying the
/// close reward from position value reverts. Bounding every reward by
/// `min_collateral`, itself above `liquidation`, keeps that gap empty.
///
/// Field names are the §5.10 rows without the redundant `keeper_` prefix
/// and `_reward` suffix, which keeps them inside Soroban's 32-byte UDT
/// field-name limit.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeeperRewards {
    /// Paid from entry escrow.
    pub open: i128,
    /// Paid from entry escrow.
    pub limit_order: i128,
    /// Paid from stored position collateral.
    pub increase: i128,
    /// Paid from stored position collateral.
    pub decrease: i128,
    /// Paid from position value.
    pub close: i128,
    /// Paid from position value.
    pub tp: i128,
    /// Paid from position value.
    pub sl: i128,
    /// Paid from action escrow.
    pub expiry: i128,
    /// Paid from position value, then LP residual, capped at what exists.
    pub liquidation: i128,
    /// Paid from position payable profit or collateral.
    pub adl: i128,
    /// Paid from LP request escrow or released assets.
    pub lp_resolve: i128,
}

/// §5.1 — configuration applying to the complete vault.
///
/// The protocol revenue share is deliberately absent: it is the exact
/// remainder after the configured LP and applicable referral shares (§6.11).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalConfig {
    /// Minimum stored collateral for a surviving position. Must exceed
    /// `keeper_rewards.liquidation` (§5.10).
    pub min_collateral: i128,
    /// Minimum time after an open or size increase before voluntary
    /// exposure removal. §8.5 gates all four exits on it.
    pub min_position_lifetime: u64,
    /// Upper bound on a limit entry's `expires_at`.
    pub max_order_lifetime_seconds: u64,
    /// §5.1 `max_market_order_lifetime_seconds` — upper bound on a market
    /// entry's `expires_at`. Shortened to fit Soroban's 32-byte UDT
    /// field-name limit.
    pub max_market_order_lifetime: u64,
    /// §3.3.2 duration used to quote each monetary minimum-borrow
    /// obligation. Bounded at `86_400` for arithmetic, not economic,
    /// reasons (§2.1.1).
    pub min_borrow_fee_seconds: u64,
    /// §3.4 half-life of the funding skew EMA, seconds (global: one memory
    /// horizon for every market).
    pub funding_half_life_seconds: u64,
    /// How old the external feed's latest observation may be before an
    /// action refuses to act on it.
    ///
    /// The protocol does not aggregate price sources — that is the feed
    /// provider's job — so this is the main price-quality control it retains,
    /// and it is governed rather than constant because it silently widens
    /// the window a trader commits against.
    pub max_price_age_seconds: u64,
    pub risk_capacity_limit_bps: u32,
    pub base_borrow_rate_bps_day: i128,
    pub max_variable_borrow_bps_day: i128,
    /// §6.11 LP share of collected opening and closing fees.
    pub fee_lp_revenue_share_bps: u32,
    /// §6.11 LP share of collected borrow — a separate split from the fee
    /// share, and one funding never uses.
    pub borrow_lp_revenue_share_bps: u32,
    /// §3.6 share of an opening or closing fee routed to the trader's
    /// referrer, carved from the protocol slice (the LP share is
    /// untouched). `0` disables referral accrual globally — a kill switch.
    /// Validated so `fee_lp + referral ≤ BPS`, keeping the protocol
    /// remainder ≥ 0.
    pub referral_fee_share_bps: u32,
    /// §12.3 delay between proposing and applying a parameter change.
    pub config_timelock_seconds: u64,
    /// Hard bound on the active-market registry and on any synchronized
    /// LP-accounting loop — what keeps §4.12's every-market checkpoint
    /// bounded.
    pub max_active_markets: u32,
    /// §5.1 `global_hard_cap_factor_limit_bps` — bound on aggregate
    /// configured hard-cap exposure across market sides. Shortened to fit
    /// Soroban's 30-character UDT field-name limit.
    pub global_hard_cap_limit_bps: u32,
    /// §6.16 growth in a latched side's positive PnL that triggers a fresh
    /// hard-cap snapshot.
    pub hard_cap_relatch_band_bps: u32,
    /// §5.10 fixed keeper rewards. Held here rather than in their own
    /// storage entry so §10.3.1's cross-parameter bounds against
    /// `min_collateral` are validated in one pass with the rest.
    pub keeper_rewards: KeeperRewards,
}

/// §5.1 — LP request policy. Global in the specification's sense (it
/// applies to the whole vault), but kept as its own record owned by the
/// vault, which is the contract that enforces every one of these bounds.
/// §10.3.1's three LP rules are validated together in one pass there; the
/// validation is never split across contracts.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpConfig {
    /// Maximum utilization permitted after an LP withdrawal.
    pub max_withdraw_utilization_bps: u32,
    /// Minimum marked-NAV-to-cash-equity factor for an ordinary
    /// share-minting deposit. A guard on the conversion arithmetic, not a
    /// market judgement (§7.17).
    pub min_deposit_nav_factor_bps: u32,
    /// Delay before an LP request can use its assigned synchronized price
    /// snapshot. Three deployment profiles in `crate::defaults`.
    pub lp_request_delay_seconds: u64,
}

/// Authoritative per-market state: the side aggregates, the funding indices,
/// the funding EMA, and the market configuration.
///
/// Soroban limits UDT field names to 30 characters, so where a doc glossary
/// term is longer the field drops the redundant qualifier and its doc
/// comment carries the full term (e.g. `receiver_backed_index_long` is the
/// doc's `receiver_backed_payer_index` for the long side).
#[contracttype]
#[derive(Clone, Debug)]
pub struct Market {
    pub long: MarketSide,
    pub short: MarketSide,
    /// Cumulative payer fee per unit of dominant-side size whose collection
    /// restores cash backing an already-accrued receiver claim (§8.2).
    pub receiver_backed_index_long: i128,
    pub receiver_backed_index_short: i128,
    /// Cumulative payer fee per unit of dominant-side size that is LP
    /// revenue on collection (§8.2).
    pub lp_backed_index_long: i128,
    pub lp_backed_index_short: i128,
    /// Cumulative funding credit per unit of light-side size (§8.2).
    pub receiver_index_long: i128,
    pub receiver_index_short: i128,
    pub current_payer_side: PayerSide,
    /// `INDEX_PRECISION`-scaled bps/day payer rate as of the last refresh.
    pub current_payer_rate: i128,
    /// §8.1 signed EMA of the instantaneous skew: a fraction of one at
    /// `INDEX_PRECISION` scale, positive when history says longs dominate.
    /// Decays toward the current skew with the global half-life.
    pub skew_ema: i128,
    pub last_funding_checkpoint: u64,
    /// §5.4 — the four carries for the divisions taken **while longs pay**.
    pub long_payer_remainders: RemainderGroup,
    /// §5.4 — the corresponding four carries **while shorts pay**. A
    /// long-payer carry and a short-payer carry are never reused by one
    /// another: they belong to different funding streams, and mixing them
    /// would move sub-unit value between directions.
    pub short_payer_remainders: RemainderGroup,
    /// §5.12 — guaranteed receiver funding attributable to **this market**
    /// and included in `Ledger::pending_receiver_funding_total`. The global
    /// total is the sum of these, not the sum of current position credits.
    pub pending_receiver_funding: i128,
    pub config: MarketConfig,
}

/// §5.4 — the four independent carries one payer direction's divisions
/// accumulate. Kept together so a stream's carries are passed and reset as
/// a unit and cannot be crossed with the opposite stream's.
#[contracttype]
#[derive(Clone, Debug, Default)]
pub struct RemainderGroup {
    /// Carry of the receiver-backed payer index division.
    pub receiver_payer_remainder: i128,
    /// Carry of the LP-backed payer index division.
    pub lp_payer_remainder: i128,
    /// Carry of the guaranteed receiver-liability accrual.
    pub receiver_liability_remainder: i128,
    /// §5.4's `receiver_distribution_remainder` — the carry of the
    /// receiver-credit distribution, shortened to fit Soroban's
    /// 30-character UDT field-name limit (the `receiver_` qualifier is
    /// already implied by the group).
    ///
    /// §4.5.1 zeroes the **opposite** stream's copy of this whenever a
    /// side's `size_open_interest` changes: the long-payer stream
    /// distributes to short receivers, so a change on the short side
    /// clears the long-payer carry. Skipping that makes §9.4's sufficiency
    /// check reachable and a receiver position unsettleable. It must not be
    /// replaced by a clamp.
    pub distribution_remainder: i128,
}

/// The three funding indices that apply to one position direction (§8.2).
#[derive(Clone, Copy, Debug)]
pub struct FundingIndices {
    pub receiver_backed_payer: i128,
    pub lp_backed_payer: i128,
    pub receiver: i128,
}

impl Market {
    /// A market with empty sides, zeroed indices, and its funding clock
    /// started at `now`.
    pub fn new(config: MarketConfig, now: u64) -> Self {
        Market {
            long: MarketSide::new(),
            short: MarketSide::new(),
            receiver_backed_index_long: 0,
            receiver_backed_index_short: 0,
            lp_backed_index_long: 0,
            lp_backed_index_short: 0,
            receiver_index_long: 0,
            receiver_index_short: 0,
            current_payer_side: PayerSide::None,
            current_payer_rate: 0,
            skew_ema: 0,
            last_funding_checkpoint: now,
            long_payer_remainders: RemainderGroup::default(),
            short_payer_remainders: RemainderGroup::default(),
            pending_receiver_funding: 0,
            config,
        }
    }

    pub fn side(&self, is_long: bool) -> &MarketSide {
        if is_long {
            &self.long
        } else {
            &self.short
        }
    }

    pub fn side_mut(&mut self, is_long: bool) -> &mut MarketSide {
        if is_long {
            &mut self.long
        } else {
            &mut self.short
        }
    }

    /// The remainder group belonging to the given payer direction.
    pub fn payer_remainders(&self, long_pays: bool) -> &RemainderGroup {
        if long_pays {
            &self.long_payer_remainders
        } else {
            &self.short_payer_remainders
        }
    }

    pub fn payer_remainders_mut(&mut self, long_pays: bool) -> &mut RemainderGroup {
        if long_pays {
            &mut self.long_payer_remainders
        } else {
            &mut self.short_payer_remainders
        }
    }

    /// The indices that apply to a position on the given direction.
    pub fn funding_indices(&self, is_long: bool) -> FundingIndices {
        if is_long {
            FundingIndices {
                receiver_backed_payer: self.receiver_backed_index_long,
                lp_backed_payer: self.lp_backed_index_long,
                receiver: self.receiver_index_long,
            }
        } else {
            FundingIndices {
                receiver_backed_payer: self.receiver_backed_index_short,
                lp_backed_payer: self.lp_backed_index_short,
                receiver: self.receiver_index_short,
            }
        }
    }
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct AccountingSnapshot {
    pub physical_cash: i128,
    pub non_lp_claims: i128,
    pub cash_lp_equity: i128,
    pub cash_shortfall: i128,
    pub required_risk_backing: i128,
    pub free_lp_capital: i128,
    pub vault_nav: i128,
    pub total_risk_units: i128,
    pub open_position_count: u64,
    /// §5.2 `restricted_market_side_count` — sides latched in `Warning`,
    /// `ADL`, or `HardCap`. Always the fresh evaluation, not the stored
    /// counter.
    pub restricted_side_count: u32,
    /// Sides in `ADL` or `HardCap` only — the count §7.17's withdrawal gate
    /// is stated over.
    ///
    /// Reported separately because `Warning` must **not** block a
    /// withdrawal, for the same reason it does not block new exposure
    /// (§6.16.1): it is a latch that makes recovery sticky, not a stop.
    /// Gating on `restricted_side_count` would freeze the whole LP queue on
    /// four sides sitting at 4.9% while nothing is actually restricted.
    pub deleveraging_side_count: u32,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LpRequestKind {
    Deposit,
    Withdrawal,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LpRequestStatus {
    Pending,
    Settled,
    Failed,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct LpRequest {
    pub id: u64,
    pub owner: Address,
    pub kind: LpRequestKind,
    pub amount: i128,
    pub request_time: u64,
    pub execute_after: u64,
    pub status: LpRequestStatus,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementStatus {
    Settled,
    Failed,
    /// §7.17 — the FIFO head is not yet resolvable. No state changes, no
    /// reward is paid, and the request stays `Pending`. Distinct from
    /// `Failed`, which is terminal and consumes the request.
    NotReady,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SettlementResult {
    pub status: SettlementStatus,
    /// Shares minted for a deposit or assets paid for a withdrawal.
    pub amount: i128,
    /// §7.17 `keeper_lp_resolve_reward` actually paid to the executor. Zero
    /// on a failed **withdrawal**, whose escrow is shares rather than cash:
    /// it releases no assets, and taking the reward in shares would
    /// confiscate part of an LP's stake for an outcome they did not cause.
    pub reward: i128,
}

/// §4.12 — a read-only quote of everything a position has accrued as of a
/// requested timestamp. Derived, never stored (§5.13).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingFeesView {
    /// Receiver-backed payer funding owed (rounds up).
    pub funding_paid_to_receivers: i128,
    /// LP-backed payer funding owed (rounds up).
    pub funding_paid_to_lps: i128,
    /// Funding credit receivable (rounds down).
    pub funding_received: i128,
    /// Borrow owed for the active window, after its monetary minimum.
    pub borrow: i128,
}

/// §12.3 — a validated configuration change waiting out its timelock.
///
/// Storing the whole proposed record rather than a field delta is what makes
/// `apply` a pure write: there is no second validation pass in which a
/// partially-applied configuration could be observed.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingGlobalConfig {
    pub config: GlobalConfig,
    pub effective_at: u64,
}

/// §12.3 — the per-market counterpart.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingMarketConfig {
    pub config: MarketConfig,
    pub effective_at: u64,
}

/// Data required during a WASM migration. Single definition for all contracts.
#[contracttype]
pub struct MigrationData {
    pub version: u32,
}

/// Pending WASM upgrade — set by `propose_upgrade`, consumed by `upgrade`
/// (cleared atomically on a successful install), or cleared by `cancel_upgrade`.
/// Single shape across every protocol contract. Contracts store it at
/// the shared `pending_upgrade` Symbol key in their own instance storage (see
/// `crate::upgrade::pending_upgrade_key`). `upgrade` refuses to install
/// unless `pending.wasm_hash` matches the supplied hash and `now >= eta`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingUpgrade {
    pub wasm_hash: BytesN<32>,
    pub eta: u64,
}
