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
    AccountingSnapshot, EntryOrder, EntryOrderParams, GlobalConfig, Market, MarketConfig,
    PendingFeesView, Position,
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

    /// Open a leveraged position (§12.1). Transfers `collateral` from
    /// `owner` — nothing is charged at open (§11.1) — and enforces the
    /// initial margin, capacity, and market-side limits.
    /// `acceptable_price` bounds the execution price (max for longs, min
    /// for shorts; `0` = no bound). Returns the new position id.
    #[allow(clippy::too_many_arguments)]
    fn open_position(
        env: Env,
        owner: Address,
        market: Symbol,
        is_long: bool,
        size: i128,
        collateral: i128,
        take_profit: i128,
        stop_loss: i128,
        acceptable_price: i128,
    ) -> u64;

    /// Add size and/or collateral to an open position (§12.1). Capitalizes
    /// all accrued fees first; added size is held to the initial margin and
    /// must pass the same risk gates as an open.
    fn increase_position(
        env: Env,
        position_id: u64,
        size_added: i128,
        collateral_added: i128,
        acceptable_price: i128,
    );

    /// Remove size and/or withdraw collateral (§12.2). `size_removed` equal
    /// to the position size is a full close and settles through the close
    /// waterfall; a partial close capitalizes accrued fees and must leave
    /// the position at or above maintenance margin. Rejected before
    /// `min_position_lifetime` has elapsed since the last increase.
    fn decrease_position(
        env: Env,
        position_id: u64,
        size_removed: i128,
        collateral_withdrawn: i128,
        acceptable_price: i128,
    );

    /// Close a position whose effective collateral (including pending fees
    /// and payable PnL) is below maintenance margin (§12.3). Open to any
    /// authenticated caller. Pays no keeper reward until the fixed
    /// `keeper_liquidation_reward` lands.
    fn liquidate_position(env: Env, caller: Address, position_id: u64);

    /// Close a profitable position on a side in the ADL or hard-cap state
    /// (KEEPER, §14). Pays no keeper reward until the fixed
    /// `keeper_adl_reward` lands.
    fn deleverage_position(env: Env, caller: Address, position_id: u64);

    /// Execute a triggered take-profit/stop-loss close (§12.4). Open to any
    /// authenticated caller. Panics `InvalidOrder` if no trigger price is
    /// crossed.
    fn execute_order(env: Env, caller: Address, position_id: u64);

    /// Set the conditional-order trigger prices (owner). `0` clears a
    /// trigger; a nonzero trigger must be on the correct side of the
    /// current price.
    fn set_tp_sl(env: Env, position_id: u64, take_profit: i128, stop_loss: i128);

    /// Place a limit/stop entry order (owner, §12.4). Storage-only — no
    /// funds move; the owner must grant the vault a token allowance covering
    /// `collateral` for the keeper to pull at fill. Returns the order id.
    fn place_entry_order(env: Env, owner: Address, market: Symbol, params: EntryOrderParams)
        -> u64;

    /// Fill an entry order whose trigger has crossed (any caller). Pulls the
    /// collateral via the owner's allowance and opens the position as a
    /// market order would. Removes the order if it is expired or unfundable;
    /// reverts (order stays) if not yet triggered, slipped, or open-blocked.
    fn execute_entry_order(env: Env, caller: Address, order_id: u64);

    /// Cancel a pending entry order (owner; permissionless once expired).
    fn cancel_entry_order(env: Env, order_id: u64);

    /// Read a pending entry order (panics `OrderNotFound` if absent).
    fn get_entry_order(env: Env, order_id: u64) -> EntryOrder;

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
    /// now (KEEPER, §10). Fee accrual is lazy; this bounds staleness.
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
    fn apply_global_config(env: Env);

    /// Register a market, or propose a change to an existing one
    /// (configuration authority, §12.3). Registration and conservative
    /// changes apply at once; everything else waits out the timelock.
    /// Bounded by `max_active_markets` and the global hard-cap factor limit.
    fn propose_market_config(env: Env, caller: Address, market: Symbol, config: MarketConfig);

    /// Apply a market proposal whose timelock has elapsed. Permissionless.
    fn apply_market_config(env: Env, market: Symbol);

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
