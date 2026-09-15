//! Shared PositionManager contract interface.
//!
//! The PositionManager is the protocol's accounting ledger: it owns the
//! market and position state, the non-LP claim totals, and every fee index.
//! The vault holds the cash; this contract decides who owns it.
//!
//! Conventions used across the trait:
//! - Prices, USD notionals, and base exposures are scaled by
//!   `constants::PRICE_PRECISION`; cash amounts use the collateral token's
//!   native decimals (identical scale on this deployment).
//! - `0` is the "none" sentinel for `take_profit`, `stop_loss`, and
//!   `acceptable_price` — a zero bound disables that check.
//! - Functions taking a `caller` verify both `require_auth` and a
//!   ConfigManager role (or a specific contract address); functions taking
//!   `owner`/`position_id` require the position owner's auth.

use soroban_sdk::{contractclient, Address, BytesN, Env, Symbol, Vec};

use crate::types::{
    AccountingSnapshot, ActionOutcome, GlobalConfig, Market, MarketConfig, OpenPayload,
    PendingAction, PendingFeesView, Position,
};

#[contractclient(name = "PositionManagerClient")]
pub trait PositionManager {
    /// One-time wiring of the vault address (ADMIN). Panics with
    /// `AlreadyInitialized` on a second call.
    fn set_vault(env: Env, caller: Address, vault: Address);

    /// Point the protocol at a different external price feed
    /// (`oracle_authority`, §12.3). Verifies the feed reports
    /// `PRICE_DECIMALS`.
    ///
    /// Rewireable rather than fixed at deploy: the protocol does not own its
    /// oracle, so replacing the provider must not require redeploying the
    /// protocol. It is the one authority whose whole job is naming the
    /// contract that supplies authenticated prices.
    fn set_price_feed(env: Env, caller: Address, price_feed: Address);

    /// The external price feed currently in use.
    fn price_feed(env: Env) -> Address;

    // -----------------------------------------------------------------
    // §7.1–7.6 Entries.
    //
    // Every price-sensitive trader action is two calls: a commitment that
    // freezes the economic inputs and the observation cursor, and a
    // settlement against a **strictly newer** observation. The settlement
    // functions return an `ActionOutcome` rather than panicking on an
    // expected business failure: a revert would undo the keeper payment and
    // the escrow refund, and hand the trader a free retry after they have
    // seen the price (§8.9).
    // -----------------------------------------------------------------

    /// §7.1 — commit a market open. Transfers `request.submitted_collateral`
    /// into action escrow, records the commit observation, and reserves
    /// nothing: no capacity, no price, no fee, no position. Binding — a
    /// market entry has no cancel. Returns the action id.
    fn create_market_open(env: Env, owner: Address, market: Symbol, request: OpenPayload) -> u64;

    /// §7.3 — commit a limit open. As `create_market_open`, but bounded by
    /// `max_order_lifetime_seconds`, requiring `trigger_price > 0`, and
    /// freezing the trigger direction against the authenticated commit
    /// price. Cancellable by the owner until expiry.
    fn create_limit_open(
        env: Env,
        owner: Address,
        market: Symbol,
        request: OpenPayload,
        trigger_price: i128,
    ) -> u64;

    /// §7.2 — settle a committed market open against a strictly newer
    /// observation. The first eligible attempt is terminal: it either opens
    /// the position or fails, pays `keeper_open_reward` from escrow, refunds
    /// the remainder, and charges no opening fee.
    fn settle_market_open(env: Env, keeper: Address, action_id: u64) -> ActionOutcome;

    /// §7.4 — settle a committed limit open. An untriggered observation
    /// returns `Pending` and does **not** consume the order; once triggered
    /// the attempt is terminal and pays `keeper_limit_order_reward`.
    fn settle_limit_open(env: Env, keeper: Address, action_id: u64) -> ActionOutcome;

    /// §7.5 — owner cancellation of a pending limit entry before expiry.
    /// Full refund, no fee, no keeper reward. Returns the refund.
    fn cancel_limit_open(env: Env, action_id: u64) -> i128;

    /// §7.6 — permissionless cleanup at or after an entry's expiry. Pays
    /// `keeper_expiry_reward` from escrow and refunds the remainder. At
    /// exactly `expires_at` execution is closed and only this succeeds.
    fn clean_expired_entry(env: Env, keeper: Address, action_id: u64);

    // -----------------------------------------------------------------
    // §7.7–7.10 Position mutations.
    // -----------------------------------------------------------------

    /// §7.7 — add collateral immediately. It adds no price exposure, so it
    /// needs no commitment: no fee, no reward, no baseline reset, and it
    /// does **not** restart the minimum-lifetime clock. A liquidatable owner
    /// may use it to rescue the position.
    fn add_collateral(env: Env, position_id: u64, amount: i128);

    /// §7.8 — commit an increase. Escrows `collateral_added`, rejects a dust
    /// add against the commitment price, and occupies the position's one
    /// pending-mutation slot. Returns the action id.
    fn create_increase(
        env: Env,
        position_id: u64,
        size_added: i128,
        collateral_added: i128,
        acceptable_price: i128,
    ) -> u64;

    /// §7.9 — commit a partial decrease of `size_removed`.
    fn create_decrease(env: Env, position_id: u64, size_removed: i128, acceptable_price: i128)
        -> u64;

    /// §7.10 — commit a full close. No size is stored: it always targets the
    /// complete remaining exposure at settlement.
    fn create_close(env: Env, position_id: u64, acceptable_price: i128) -> u64;

    /// §7.8 — settle a committed increase.
    fn settle_increase(env: Env, keeper: Address, action_id: u64) -> ActionOutcome;

    /// §7.9 — settle a committed decrease.
    fn settle_decrease(env: Env, keeper: Address, action_id: u64) -> ActionOutcome;

    /// §7.10 — settle a committed close.
    fn settle_close(env: Env, keeper: Address, action_id: u64) -> ActionOutcome;

    // -----------------------------------------------------------------
    // §7.11–7.14 Triggers and forced actions.
    // -----------------------------------------------------------------

    /// §7.11 — attach or replace a take-profit. Records a fresh commitment
    /// cursor, so the instruction cannot execute on the observation that
    /// armed it.
    fn set_take_profit(env: Env, position_id: u64, trigger_price: i128, acceptable_price: i128);
    fn clear_take_profit(env: Env, position_id: u64);
    /// §7.12 — the stop-loss mirror.
    fn set_stop_loss(env: Env, position_id: u64, trigger_price: i128, acceptable_price: i128);
    fn clear_stop_loss(env: Env, position_id: u64);

    /// §7.11 — execute a crossed take-profit. Closes the position in full
    /// and pays only `keeper_tp_reward`. A crossed trigger whose exit bound
    /// fails leaves the instruction attached and pays nothing (§8.7).
    fn execute_take_profit(env: Env, keeper: Address, position_id: u64) -> ActionOutcome;

    /// §7.12 — execute a crossed stop-loss. Pays only `keeper_sl_reward`.
    fn execute_stop_loss(env: Env, keeper: Address, position_id: u64) -> ActionOutcome;

    /// §7.13 — liquidate a position whose effective collateral has fallen to
    /// its threshold. Permissionless. One snapshot serves eligibility and
    /// settlement. Pays `keeper_liquidation_reward` from position value,
    /// then LP residual for the gap, capped at what exists; no closing fee.
    fn liquidate_position(env: Env, keeper: Address, position_id: u64);

    /// §7.14 — deleverage one profitable position on a side the current book
    /// puts in `ADL` or `HardCap`. Permissionless: the state gate bounds the
    /// mechanism, not an allowlist. Fixed `keeper_adl_reward`, no closing
    /// fee.
    fn execute_adl(env: Env, keeper: Address, position_id: u64) -> ActionOutcome;

    /// Read a pending action (panics `ActionNotFound` if absent or
    /// consumed).
    fn get_pending_action(env: Env, action_id: u64) -> PendingAction;

    /// Register a referral code (owner). First-come; the code owner is
    /// immutable, and one address may own several codes. Panics
    /// `ReferralCodeTaken` if the code exists.
    fn register_referral_code(env: Env, owner: Address, code: Symbol);

    /// Point the caller at a referrer via that referrer's code (§11.1).
    /// Freely re-settable; a trader cannot refer themselves.
    fn set_referrer(env: Env, trader: Address, code: Symbol);

    /// Withdraw the caller's accrued referral rewards (§11.1).
    /// Conservation-checked, so it is blocked during a cash shortfall.
    fn claim_referral(env: Env, referrer: Address);

    /// The referrer a trader is attached to, if any.
    fn get_referrer(env: Env, trader: Address) -> Option<Address>;

    /// The owner of a referral code, if it is registered.
    fn referral_code_owner(env: Env, code: Symbol) -> Option<Address>;

    /// A referrer's accrued unclaimed referral rewards.
    fn referral_balance(env: Env, referrer: Address) -> i128;

    /// Total accrued unclaimed referral rewards across all referrers.
    fn referral_claimable_total(env: Env) -> i128;

    /// Checkpoint the global indices and one market's funding indices to
    /// now. Fee accrual is lazy; this bounds staleness. Permissionless
    /// (§7.0): a checkpoint pays no reward and moves no value between
    /// parties, so there is nothing for an allowlist to protect.
    fn update_indices(env: Env, caller: Address, market: Symbol);

    /// Propose a global configuration change (configuration authority,
    /// §12.3). Validated immediately and stored with
    /// `effective_at = now + config_timelock_seconds`. A purely
    /// conservative change — lowering `risk_capacity_limit_bps` with
    /// nothing else altered — is exempt and applies at once.
    fn propose_global_config(env: Env, caller: Address, config: GlobalConfig);

    /// Apply a global proposal whose timelock has elapsed. Permissionless:
    /// the authorization happened at proposal and the delay is the
    /// protection. Checkpoints under the old values before storing (§10.3.3).
    fn apply_global_config(env: Env, caller: Address);

    /// Register a market, or propose a change to an existing one
    /// (configuration authority, §12.3). Registration and conservative
    /// changes apply at once; everything else waits out the timelock.
    /// Bounded by `max_active_markets` and the global hard-cap factor limit.
    fn propose_market_config(env: Env, caller: Address, market: Symbol, config: MarketConfig);

    /// Apply a market proposal whose timelock has elapsed. Permissionless.
    fn apply_market_config(env: Env, caller: Address, market: Symbol);

    /// §7.18 — remove a market from the active registry (configuration
    /// authority). Requires both sides empty of open interest, base
    /// exposure, and risk units, no pending receiver-funding liability, and
    /// both sides in `Normal`.
    ///
    /// The market's accounting record is **kept**: indices and the
    /// checkpoint timestamp are retained, never reset, so a later
    /// re-registration cannot rewind an index a historical position was
    /// priced against. Re-registering is `propose_market_config` on the same
    /// symbol.
    fn deregister_market(env: Env, caller: Address, market: Symbol);

    /// Block new opens/increases on one market (PAUSER). Existing positions
    /// keep accruing and can always decrease, close, or be liquidated.
    fn disable_market(env: Env, caller: Address, market: Symbol);
    fn enable_market(env: Env, caller: Address, market: Symbol);
    fn is_market_disabled(env: Env, market: Symbol) -> bool;

    /// LP-settlement snapshot (vault only, §7.17): checkpoints global
    /// accrual, prices every active market from the external feed inside
    /// this transaction, evaluates and persists every side's risk state
    /// against those prices, and returns the accounting snapshot the
    /// settlement decides against.
    ///
    /// Reading each market here rather than accepting a published round is
    /// what removes the liveness dependency the round carried: there is no
    /// separate publication step that can stop while positions keep
    /// trading.
    fn prepare_lp_snapshot(env: Env, caller: Address, physical_cash: i128) -> AccountingSnapshot;

    /// Recompute the borrow rate from current utilization (vault only,
    /// called after vault cash moved).
    fn refresh_borrow_rate(env: Env, caller: Address, physical_cash: i128);

    /// Whether new LP requests may be created: no cash shortfall and no
    /// side in a restricted risk state (vault only, §14).
    fn can_create_lp_request(env: Env, caller: Address, physical_cash: i128) -> bool;

    /// Read-only accounting snapshot at current feed prices — no
    /// risk-state transitions are persisted and no accrual checkpoint runs.
    fn accounting_snapshot(env: Env, physical_cash: i128) -> AccountingSnapshot;

    fn get_position(env: Env, position_id: u64) -> Position;

    /// §4.12 — quote a position's accrued fees as of `now` without a
    /// state-changing checkpoint. The global index and the market's funding
    /// indices are advanced on in-memory copies; nothing is written.
    ///
    /// Reading the last stored indices instead would understate fees after
    /// time has elapsed. A quote immediately followed by settlement reports
    /// the same amounts unless another transaction changes state first.
    fn pending_fees(env: Env, position_id: u64, now: u64) -> PendingFeesView;
    fn get_market(env: Env, market: Symbol) -> Market;
    fn active_markets(env: Env) -> Vec<Symbol>;
    fn global_config(env: Env) -> GlobalConfig;
    /// The guaranteed receiver-funding liability (§8.3).
    fn pending_receiver_funding_total(env: Env) -> i128;
    fn protocol_claimable_total(env: Env) -> i128;
    /// Complete non-LP claims on the vault's physical cash (§4.2).
    fn non_lp_claims(env: Env) -> i128;

    /// Pay out protocol revenue (`protocol_recipient`, §12.3).
    /// Conservation-checked against the remaining claims, so it is blocked
    /// during a cash shortfall.
    fn claim_protocol(env: Env, caller: Address, recipient: Address, amount: i128);

    /// Transfer cash into the vault without minting shares (§15.2). Open to
    /// anyone; the cure for a cash shortfall.
    fn recapitalize(env: Env, contributor: Address, amount: i128);

    /// §12.2 operational pause: blocks every path that adds exposure and
    /// none that removes it. Accrual clocks keep running (§12.2) and
    /// closes, liquidations, and ADL stay available. `pause_authority`.
    fn pause(env: Env, caller: Address);
    /// Clear the pause. A **separate** authority from `pause`: pausing is a
    /// fast safety action, unpausing re-admits risk (§12.3).
    fn unpause(env: Env, caller: Address);

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);
    fn cancel_upgrade(env: Env, caller: Address);

    // §12.4 — every persistent entry is extendable permissionlessly. A
    // position whose owner has gone quiet must still be liquidatable, and a
    // referral balance must survive its owner's inactivity; letting either
    // expire destroys a claim, which no rule in §9 permits.

    /// Re-extend a position entry's storage TTL. Open to anyone.
    fn bump_position(env: Env, position_id: u64);
    /// Re-extend a pending action's storage TTL. Open to anyone.
    fn bump_pending_action(env: Env, action_id: u64);
    /// Re-extend a market's storage TTL. Open to anyone.
    fn bump_market_entry(env: Env, market: Symbol);
    /// Re-extend referral entries' storage TTL. Open to anyone.
    fn bump_referral_entry(env: Env, code: Symbol, trader: Address, referrer: Address);
}
