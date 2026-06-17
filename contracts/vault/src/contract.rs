
use soroban_sdk::{
    contract, contractimpl, panic_with_error, Address, BytesN, Env, MuxedAddress, String,
};

use interfaces::{ConfigManagerClient, MigrationData, TimelockedUpgradeable, UpgradeFailure};

use stellar_contract_utils::{
    math::Rounding,
    upgradeable::{complete_migration, ensure_can_complete_migration},
};
use stellar_tokens::{
    fungible::{Base, FungibleToken},
    vault::{emit_deposit, emit_withdraw, FungibleVault, Vault},
};

use crate::errors::VaultError;
use crate::events as vault_events;
use crate::logic as vault_logic;
use crate::storage as vault_storage;

#[contract]
pub struct VaultContract;

// ---------------------------------------------------------------------------
// SEP-41 token interface — auto-implemented by OZ Vault (which extends Base)
// ---------------------------------------------------------------------------
#[contractimpl(contracttrait)]
impl FungibleToken for VaultContract {
    type ContractType = Vault;

    fn decimals(e: &Env) -> u32 {
        Vault::decimals(e)
    }

    /// Override to propagate the sender's remaining lockup onto a recipient
    /// who held no shares before the transfer. Without this, an LP could
    /// circumvent the cooldown by transferring LP shares to a fresh address
    /// that then withdraws. Recipients with an existing balance keep their
    /// own expiry — see `propagate_lockup_on_transfer`.
    fn transfer(e: &Env, from: Address, to: MuxedAddress, amount: i128) {
        let to_addr = to.address();
        let to_pre_balance = Base::balance(e, &to_addr);
        Base::transfer(e, &from, &to, amount);
        vault_logic::propagate_lockup_on_transfer(e, &from, &to_addr, to_pre_balance);
    }

    /// Same lockup-propagation guarantee for the allowance-based path.
    fn transfer_from(e: &Env, spender: Address, from: Address, to: Address, amount: i128) {
        let to_pre_balance = Base::balance(e, &to);
        Base::transfer_from(e, &spender, &from, &to, amount);
        vault_logic::propagate_lockup_on_transfer(e, &from, &to, to_pre_balance);
    }
}

// ---------------------------------------------------------------------------
// ERC-4626 vault interface — delegates to OZ Vault with custom wrappers
// ---------------------------------------------------------------------------
#[contractimpl]
// Every conversion below prices shares against the LP-owned basis
// (`lp_total_assets` = raw custody − unclaimed_fees − max(0, net trader
// PnL)), not the raw token balance. The raw balance includes the dev+staker
// fee buffer and the liability to pay winning traders; pricing against it
// would let a withdrawing LP skim those buffers from co-LPs. The share math
// itself is the OZ virtual-offset formula — see `logic::assets_to_shares` /
// `logic::shares_to_assets`. Raw custody remains visible via
// `Vault::total_assets` internally (invariant checks, indexer snapshots):
// raw = lp_total_assets + unclaimed_fees + max(0, net_pnl).
impl FungibleVault for VaultContract {
    fn query_asset(e: &Env) -> Address {
        Vault::query_asset(e)
    }

    fn total_assets(e: &Env) -> i128 {
        vault_logic::lp_total_assets(e)
    }

    fn convert_to_shares(e: &Env, assets: i128) -> i128 {
        vault_logic::assets_to_shares(e, assets, Rounding::Floor)
    }

    fn convert_to_assets(e: &Env, shares: i128) -> i128 {
        vault_logic::shares_to_assets(e, shares, Rounding::Floor)
    }

    fn max_deposit(e: &Env, receiver: Address) -> i128 {
        if vault_storage::get_paused(e) {
            return 0;
        }
        Vault::max_deposit(e, receiver)
    }

    fn preview_deposit(e: &Env, assets: i128) -> i128 {
        vault_logic::assets_to_shares(e, assets, Rounding::Floor)
    }

    fn deposit(e: &Env, assets: i128, receiver: Address, from: Address, operator: Address) -> i128 {
        vault_logic::require_not_paused(e);
        vault_logic::require_initialized(e);
        if receiver != from || operator != from {
            panic_with_error!(e, VaultError::DepositMustBeSelf);
        }
        if assets <= 0 {
            panic_with_error!(e, VaultError::ZeroAmount);
        }
        operator.require_auth();
        vault_logic::record_lockup(e, &receiver);
        // Shares are priced on the LP basis BEFORE the asset transfer lands.
        let shares = vault_logic::assets_to_shares(e, assets, Rounding::Floor);
        Vault::deposit_internal(e, &receiver, assets, shares, &from, &operator);
        emit_deposit(e, &operator, &from, &receiver, assets, shares);
        vault_events::TotalAssetsUpdate {
            new_total_assets: Vault::total_assets(e),
        }
        .publish(e);
        shared::bump_instance_ttl(e);
        shares
    }

    fn max_mint(e: &Env, receiver: Address) -> i128 {
        if vault_storage::get_paused(e) {
            return 0;
        }
        Vault::max_mint(e, receiver)
    }

    fn preview_mint(e: &Env, shares: i128) -> i128 {
        vault_logic::shares_to_assets(e, shares, Rounding::Ceil)
    }

    fn mint(e: &Env, shares: i128, receiver: Address, from: Address, operator: Address) -> i128 {
        vault_logic::require_not_paused(e);
        vault_logic::require_initialized(e);
        if receiver != from || operator != from {
            panic_with_error!(e, VaultError::DepositMustBeSelf);
        }
        if shares <= 0 {
            panic_with_error!(e, VaultError::ZeroAmount);
        }
        operator.require_auth();
        vault_logic::record_lockup(e, &receiver);
        // Asset cost is priced on the LP basis, rounded up in the vault's favor.
        let assets = vault_logic::shares_to_assets(e, shares, Rounding::Ceil);
        Vault::deposit_internal(e, &receiver, assets, shares, &from, &operator);
        // Mint and deposit collapse to one event shape.
        emit_deposit(e, &operator, &from, &receiver, assets, shares);
        vault_events::TotalAssetsUpdate {
            new_total_assets: Vault::total_assets(e),
        }
        .publish(e);
        shared::bump_instance_ttl(e);
        assets
    }

    fn max_withdraw(e: &Env, owner: Address) -> i128 {
        if vault_storage::get_paused(e) {
            return 0;
        }
        let user_assets =
            vault_logic::shares_to_assets(e, Base::balance(e, &owner), Rounding::Floor);
        let free = vault_logic::free_liquidity(e);
        core::cmp::min(user_assets, free)
    }

    fn preview_withdraw(e: &Env, assets: i128) -> i128 {
        vault_logic::assets_to_shares(e, assets, Rounding::Ceil)
    }

    fn withdraw(
        e: &Env,
        assets: i128,
        receiver: Address,
        owner: Address,
        operator: Address,
    ) -> i128 {
        vault_logic::require_not_paused(e);
        vault_logic::require_initialized(e);
        vault_logic::require_lockup_elapsed(e, &owner);
        vault_logic::require_fresh_pnl_sync(e);
        vault_logic::require_free_liquidity(e, assets);
        operator.require_auth();
        // Shares burned are priced on the LP basis, rounded up in the
        // vault's favor.
        let shares = vault_logic::assets_to_shares(e, assets, Rounding::Ceil);
        Vault::withdraw_internal(e, &receiver, &owner, assets, shares, &operator);
        emit_withdraw(e, &operator, &receiver, &owner, assets, shares);
        vault_events::TotalAssetsUpdate {
            new_total_assets: Vault::total_assets(e),
        }
        .publish(e);
        shared::bump_instance_ttl(e);
        shares
    }

    fn max_redeem(e: &Env, owner: Address) -> i128 {
        if vault_storage::get_paused(e) {
            return 0;
        }
        let max_w = Self::max_withdraw(e, owner.clone());
        vault_logic::assets_to_shares(e, max_w, Rounding::Floor)
    }

    fn preview_redeem(e: &Env, shares: i128) -> i128 {
        vault_logic::shares_to_assets(e, shares, Rounding::Floor)
    }

    fn redeem(e: &Env, shares: i128, receiver: Address, owner: Address, operator: Address) -> i128 {
        vault_logic::require_not_paused(e);
        vault_logic::require_initialized(e);
        vault_logic::require_lockup_elapsed(e, &owner);
        vault_logic::require_fresh_pnl_sync(e);
        let assets = vault_logic::shares_to_assets(e, shares, Rounding::Floor);
        vault_logic::require_free_liquidity(e, assets);
        operator.require_auth();
        Vault::withdraw_internal(e, &receiver, &owner, assets, shares, &operator);
        // Redeem and withdraw collapse to one event shape.
        emit_withdraw(e, &operator, &receiver, &owner, assets, shares);
        vault_events::TotalAssetsUpdate {
            new_total_assets: Vault::total_assets(e),
        }
        .publish(e);
        shared::bump_instance_ttl(e);
        assets
    }
}

// ---------------------------------------------------------------------------
// Custom vault methods
// ---------------------------------------------------------------------------
#[contractimpl]
impl VaultContract {
    /// Atomic-with-deploy initialization (Soroban constructor). Binds the
    /// asset, the linked ConfigManager, and — critically — the trusted
    /// `position_manager` once, inside the deploy transaction. This closes the
    /// front-running window in which an attacker could initialize an
    /// uninitialized vault with their own `position_manager` and drain LP
    /// funds via `pay_profit` / `claim_fees_to`.
    pub fn __constructor(
        env: Env,
        asset: Address,
        config_manager: Address,
        position_manager: Address,
    ) {
        Vault::set_asset(&env, asset);
        Vault::set_decimals_offset(&env, 6);
        Base::set_metadata(
            &env,
            Vault::decimals(&env),
            String::from_str(&env, "Stellars LP"),
            String::from_str(&env, "sLP"),
        );

        vault_storage::set_config_manager(&env, &config_manager);
        vault_storage::set_position_manager(&env, &position_manager);
        vault_storage::set_reserved_usdc(&env, 0);
        vault_storage::set_unclaimed_fees(&env, 0);
        vault_storage::set_net_global_trader_pnl(&env, 0);
        vault_storage::set_paused(&env, false);
        vault_storage::set_initialized(&env);

        shared::bump_instance_ttl(&env);
    }

    /// Pay `amount` from the vault to `trader` to settle a profitable close.
    /// Loss settlement does NOT route through here — see ADR-0001.
    pub fn pay_profit(
        env: Env,
        caller: Address,
        trader: Address,
        amount: i128,
    ) {
        vault_logic::require_initialized(&env);
        vault_logic::require_position_manager(&env, &caller);

        if amount <= 0 {
            panic_with_error!(&env, VaultError::ZeroAmount);
        }

        vault_logic::require_payout_liquidity(&env, amount);
        let asset = Vault::query_asset(&env);
        let vault_addr = env.current_contract_address();
        vault_logic::transfer_asset(&env, &asset, &vault_addr, &trader, amount);

        let new_total_assets = Vault::total_assets(&env);
        vault_events::PayProfit {
            trader: trader.clone(),
            amount,
            new_total_assets,
        }
        .publish(&env);
        shared::bump_instance_ttl(&env);
    }

    pub fn reserve_liquidity(env: Env, caller: Address, amount: i128) {
        vault_logic::require_initialized(&env);
        vault_logic::require_position_manager(&env, &caller);

        if amount <= 0 {
            panic_with_error!(&env, VaultError::ZeroAmount);
        }

        let current = vault_storage::get_reserved_usdc(&env);
        let new_reserved = current + amount;
        let total = Vault::total_assets(&env);
        let unclaimed = vault_storage::get_unclaimed_fees(&env);
        // Upholds the `reserved + unclaimed_fees <= total_assets` invariant
        // that `accrue_fees` and PM's utilization basis rely on: positions
        // may never be reserved against the fee buffer.
        if new_reserved + unclaimed > total {
            panic_with_error!(&env, VaultError::ReservationExceedsTotalAssets);
        }
        vault_storage::set_reserved_usdc(&env, new_reserved);
        vault_events::Reserve { amount, new_total: new_reserved }.publish(&env);
        shared::bump_instance_ttl(&env);
    }

    pub fn release_liquidity(env: Env, caller: Address, amount: i128) {
        vault_logic::require_initialized(&env);
        vault_logic::require_position_manager(&env, &caller);

        if amount <= 0 {
            panic_with_error!(&env, VaultError::ZeroAmount);
        }

        let current = vault_storage::get_reserved_usdc(&env);
        if amount > current {
            panic_with_error!(&env, VaultError::InsufficientFreeLiquidity);
        }
        let new_reserved = current - amount;
        vault_storage::set_reserved_usdc(&env, new_reserved);
        vault_events::Release { amount, new_total: new_reserved }.publish(&env);
        shared::bump_instance_ttl(&env);
    }

    pub fn update_net_pnl(env: Env, caller: Address, pnl: i128) {
        vault_logic::require_initialized(&env);
        vault_logic::require_position_manager(&env, &caller);
        vault_storage::set_net_global_trader_pnl(&env, pnl);
        vault_storage::set_last_pnl_sync(&env, env.ledger().timestamp());
        vault_events::UpdateNetPnl { pnl }.publish(&env);
        shared::bump_instance_ttl(&env);
    }

    /// Notify the vault that PositionManager has just transferred `amount`
    /// USDC of seized/loss-settlement collateral directly into the vault's
    /// wallet. This call does NOT move tokens, but it DOES verify the
    /// on-chain delta — `post - pre` must equal `amount`, otherwise PM and
    /// Vault have diverged and we panic. See ADR-0001.
    pub fn record_absorbed_collateral(
        env: Env,
        caller: Address,
        trader: Address,
        amount: i128,
        pre_balance: i128,
    ) {
        vault_logic::require_initialized(&env);
        vault_logic::require_position_manager(&env, &caller);
        if amount <= 0 {
            panic_with_error!(&env, VaultError::ZeroAmount);
        }
        let post = Vault::total_assets(&env);
        if post.saturating_sub(pre_balance) != amount {
            panic_with_error!(&env, VaultError::AbsorbedCollateralMismatch);
        }
        vault_events::AbsorbedCollateral {
            trader,
            amount,
            new_total_assets: post,
        }
        .publish(&env);
        shared::bump_instance_ttl(&env);
    }

    /// Books PM-pushed revenue into `unclaimed_fees`, clamped so
    /// `unclaimed_fees + reserved_usdc` never exceeds `total_assets` — fee
    /// claims can only ever be tagged against capital the vault holds. A
    /// clamped accrual emits `FeeAccrualClamped` for monitoring instead of
    /// reverting: this runs inside PM's close/liquidation paths, which must
    /// never fail on fee bookkeeping.
    ///
    /// Emits `TotalAssetsUpdate` alongside `AccrueFees`. PM's `recv_revenue`
    /// pushes fee USDC into the vault via a raw token transfer immediately
    /// before this call, and no other vault entrypoint witnesses that
    /// transfer — without the snapshot, off-chain indexers would lose the
    /// LP slice (`fee - non_lp_slice`) on every accrual.
    pub fn accrue_fees(env: Env, caller: Address, amount: i128) {
        vault_logic::require_initialized(&env);
        vault_logic::require_position_manager(&env, &caller);

        if amount <= 0 {
            panic_with_error!(&env, VaultError::ZeroAmount);
        }

        let current = vault_storage::get_unclaimed_fees(&env);
        let reserved = vault_storage::get_reserved_usdc(&env);
        let total_assets = Vault::total_assets(&env);
        let headroom = total_assets - reserved - current;
        let accrued = amount.min(if headroom > 0 { headroom } else { 0 });
        if accrued < amount {
            vault_events::FeeAccrualClamped {
                requested: amount,
                accrued,
            }
            .publish(&env);
        }
        let new_total = current + accrued;
        vault_storage::set_unclaimed_fees(&env, new_total);
        vault_events::AccrueFees {
            amount: accrued,
            new_total,
        }
        .publish(&env);
        vault_events::TotalAssetsUpdate {
            new_total_assets: total_assets,
        }
        .publish(&env);
        shared::bump_instance_ttl(&env);
    }

    pub fn claim_fees(env: Env, caller: Address, recipient: Address) {
        vault_logic::require_initialized(&env);
        vault_logic::require_admin(&env, &caller);

        let fees = vault_storage::get_unclaimed_fees(&env);
        if fees <= 0 {
            panic_with_error!(&env, VaultError::ZeroAmount);
        }

        let asset = Vault::query_asset(&env);
        let vault_addr = env.current_contract_address();
        vault_logic::transfer_asset(&env, &asset, &vault_addr, &recipient, fees);
        vault_storage::set_unclaimed_fees(&env, 0);
        vault_events::ClaimFees { amount: fees, recipient: recipient.clone() }.publish(&env);
        shared::bump_instance_ttl(&env);
    }

    pub fn claim_fees_to(env: Env, caller: Address, recipient: Address, amount: i128) {
        vault_logic::require_initialized(&env);
        vault_logic::require_position_manager(&env, &caller);

        if amount <= 0 {
            panic_with_error!(&env, VaultError::ZeroAmount);
        }

        let fees = vault_storage::get_unclaimed_fees(&env);
        if amount > fees {
            panic_with_error!(&env, VaultError::InsufficientFees);
        }

        let asset = Vault::query_asset(&env);
        let vault_addr = env.current_contract_address();
        vault_logic::transfer_asset(&env, &asset, &vault_addr, &recipient, amount);
        vault_storage::set_unclaimed_fees(&env, fees - amount);
        vault_events::ClaimFeesTo { amount, new_total: fees - amount, recipient: recipient.clone() }.publish(&env);
        shared::bump_instance_ttl(&env);
    }

    pub fn pause(env: Env, caller: Address) {
        vault_logic::require_initialized(&env);
        vault_logic::require_pauser(&env, &caller);
        vault_storage::set_paused(&env, true);
        vault_events::Pause { is_paused: true, caller: caller.clone() }.publish(&env);
        shared::bump_instance_ttl(&env);
    }

    pub fn unpause(env: Env, caller: Address) {
        vault_logic::require_initialized(&env);
        vault_logic::require_pauser(&env, &caller);
        vault_storage::set_paused(&env, false);
        vault_events::Pause { is_paused: false, caller: caller.clone() }.publish(&env);
        shared::bump_instance_ttl(&env);
    }

    pub fn free_liquidity(env: Env) -> i128 {
        vault_logic::require_initialized(&env);
        vault_logic::free_liquidity(&env)
    }

    /// Total assets minus only the fee buffer — PnL is excluded so consumers
    /// (PM's utilization gate) are not subject to mark-price feedback into
    /// the utilization denominator. LP-facing flows still use `free_liquidity`.
    pub fn total_assets_excl_pnl(env: Env) -> i128 {
        vault_logic::require_initialized(&env);
        vault_logic::total_assets_excl_pnl(&env)
    }

    pub fn reserved_usdc(env: Env) -> i128 {
        vault_logic::require_initialized(&env);
        vault_storage::get_reserved_usdc(&env)
    }

    /// Accrued non-LP revenue awaiting `claim_fees` / `claim_fees_to`. Exposed
    /// so tests can reconcile counter movement against token-side transfers
    /// without inferring via subtraction.
    pub fn unclaimed_fees(env: Env) -> i128 {
        vault_logic::require_initialized(&env);
        vault_storage::get_unclaimed_fees(&env)
    }

    /// Net unrealized PnL across all open trader positions, as last synced by
    /// PM via `update_net_pnl`. Realized PnL is intentionally NOT included —
    /// it has already moved physically through `pay_profit` /
    /// `record_absorbed_collateral` and is reflected directly in `total_assets`.
    pub fn net_global_trader_pnl(env: Env) -> i128 {
        vault_logic::require_initialized(&env);
        vault_storage::get_net_global_trader_pnl(&env)
    }

    /// Returns the unix timestamp at which `user` may next withdraw/redeem.
    /// Returns 0 if `user` has never deposited (no lockup recorded).
    pub fn lockup_expires_at(env: Env, user: Address) -> u64 {
        vault_storage::get_lockup_expires_at(&env, &user).unwrap_or(0)
    }

    pub fn bump_vault_state(env: Env) {
        shared::bump_instance_ttl(&env);
    }

    /// Propose a WASM upgrade. UPGRADER role only. Records `{wasm_hash, eta}`
    /// where `eta = now + timelock` so `upgrade` can refuse to install a
    /// different hash or fire before `eta`.
    pub fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        vault_logic::require_initialized(&env);
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
        shared::bump_instance_ttl(&env);
    }

    /// PAUSER veto of a pending upgrade.
    pub fn cancel_upgrade(env: Env, caller: Address) {
        vault_logic::require_initialized(&env);
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
        shared::bump_instance_ttl(&env);
    }

    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        vault_logic::require_initialized(&env);
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, migration_data: MigrationData, operator: Address) {
        vault_logic::require_initialized(&env);
        vault_logic::require_upgrader(&env, &operator);
        ensure_can_complete_migration(&env);
        Self::_migrate(&env, &migration_data);
        complete_migration(&env);
        shared::bump_instance_ttl(&env);
    }
}

impl VaultContract {
    pub(crate) fn _migrate(env: &Env, data: &MigrationData) {
        vault_storage::save_version(env, data.version);
    }
}

// ---------------------------------------------------------------------------
// TimelockedUpgradeable impl — hooks supply the contract-specific bits.
// ---------------------------------------------------------------------------
impl TimelockedUpgradeable for VaultContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        vault_logic::require_upgrader(env, caller);
    }
    fn _require_executor(env: &Env, caller: &Address) {
        vault_logic::require_upgrader(env, caller);
    }
    fn _require_canceller(env: &Env, caller: &Address) {
        vault_logic::require_pauser(env, caller);
    }
    fn _timelock_seconds(env: &Env) -> u64 {
        let config_mgr = vault_storage::get_config_manager(env);
        ConfigManagerClient::new(env, &config_mgr).get_upgrade_timelock()
    }
    fn _panic_with_upgrade_error(env: &Env, err: UpgradeFailure) -> ! {
        match err {
            UpgradeFailure::NoPendingUpgrade => {
                panic_with_error!(env, VaultError::NoPendingUpgrade)
            }
            UpgradeFailure::TimelockNotElapsed => {
                panic_with_error!(env, VaultError::UpgradeTimelockNotElapsed)
            }
            UpgradeFailure::HashMismatch => {
                panic_with_error!(env, VaultError::UpgradeHashMismatch)
            }
        }
    }
}
