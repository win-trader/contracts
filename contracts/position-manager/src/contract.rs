use shared::constants::{
    BPS, INDEX_PRECISION, PRECISION, ROLE_ADMIN, ROLE_KEEPER, ROLE_PAUSER, ROLE_UPGRADER,
    SECONDS_PER_DAY,
};
use shared::{
    AccountingSnapshot, ConfigManagerClient, GlobalConfig, MarketConfig, MarketInfo, MarketSide,
    MigrationData, OracleRound, OracleRouterClient, Position, PositionManager, RiskState,
    TimelockedUpgradeable, UpgradeFailure, VaultClient,
};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol, Vec};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

use crate::errors::PositionManagerError;
use crate::{events, math, storage};

#[contract]
pub struct PositionManagerContract;

fn require_role(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    if !shared::has_role(env, &storage::config_manager(env), role, caller) {
        panic_with_error!(env, PositionManagerError::Unauthorized);
    }
    shared::bump_instance_ttl(env);
}

fn require_initialized(env: &Env) {
    if storage::get::<bool>(env, &storage::Key::Initialized) != Some(true) {
        panic_with_error!(env, PositionManagerError::NotInitialized);
    }
}

fn require_vault(env: &Env, caller: &Address) {
    caller.require_auth();
    if storage::get::<Address>(env, &storage::Key::Vault) != Some(caller.clone()) {
        panic_with_error!(env, PositionManagerError::InvalidCaller);
    }
}

fn validate_global(env: &Env, c: &GlobalConfig) {
    let split = c.lp_revenue_share_bps as u64 + c.risk_keeper_revenue_share_bps as u64;
    if c.min_collateral <= 0
        || c.risk_capacity_limit_bps == 0
        || c.risk_capacity_limit_bps > BPS as u32
        || c.base_borrow_rate_bps_day < 0
        || c.base_borrow_rate_bps_day > BPS
        || c.max_variable_borrow_bps_day < 0
        || c.max_variable_borrow_bps_day > BPS
        || split > BPS as u64
        || c.hard_cap_factor_limit_bps > BPS as u32
        || c.max_adl_reward < 0
        || c.max_insolvent_touch_reward < 0
        || c.max_active_markets == 0
    {
        panic_with_error!(env, PositionManagerError::InvalidConfig);
    }
}

fn validate_market(env: &Env, c: &MarketConfig) {
    if c.open_fee_low_bps > c.open_fee_high_bps
        || c.open_fee_high_bps > BPS as u32
        || c.max_funding_rate_bps_day < 0
        || c.max_funding_rate_bps_day > BPS
        || c.market_risk_factor_bps == 0
        || c.market_risk_factor_bps > BPS as u32
        || c.recovery_pnl_factor_bps >= c.warning_pnl_factor_bps
        || c.warning_pnl_factor_bps >= c.adl_pnl_factor_bps
        || c.adl_pnl_factor_bps >= c.hard_cap_pnl_factor_bps
        || c.hard_cap_pnl_factor_bps > BPS as u32
        || c.maintenance_margin_bps == 0
        || c.maintenance_margin_bps > BPS as u32
        || c.liquidation_reward_bps > BPS as u32
        || c.adl_reward_bps > BPS as u32
        || c.max_long_size_open_interest <= 0
        || c.max_short_size_open_interest <= 0
        || c.max_long_base_exposure <= 0
        || c.max_short_base_exposure <= 0
    {
        panic_with_error!(env, PositionManagerError::InvalidConfig);
    }
}

fn empty_side() -> MarketSide {
    MarketSide {
        size_open_interest: 0,
        base_exposure: 0,
        stored_collateral_total: 0,
        risk_units: 0,
        risk_state: RiskState::Normal,
    }
}

fn empty_market(config: MarketConfig, now: u64) -> MarketInfo {
    MarketInfo {
        long: empty_side(),
        short: empty_side(),
        recv_payer_index_long: 0,
        recv_payer_index_short: 0,
        lp_backed_payer_index_long: 0,
        lp_backed_payer_index_short: 0,
        receiver_index_long: 0,
        receiver_index_short: 0,
        current_payer_side: 0,
        current_payer_rate: 0,
        receiver_flow_per_second: 0,
        current_lp_flow_per_second: 0,
        last_funding_checkpoint: now,
        receiver_payer_remainder: 0,
        lp_payer_remainder: 0,
        receiver_index_remainder: 0,
        receiver_flow_remainder: 0,
        config,
    }
}

fn claim_total(env: &Env) -> i128 {
    let mut total = storage::get_i128(env, &storage::Key::StoredCollateralTotal);
    total = math::add(
        env,
        total,
        storage::get_i128(env, &storage::Key::PendingReceiverFundingTotal),
    );
    total = math::add(
        env,
        total,
        storage::get_i128(env, &storage::Key::ExecutionBudgetTotal),
    );
    total = math::add(
        env,
        total,
        storage::get_i128(env, &storage::Key::ProtocolClaimableTotal),
    );
    math::add(
        env,
        total,
        storage::get_i128(env, &storage::Key::RiskKeeperReserveTotal),
    )
}

fn cash_equity(env: &Env, physical_cash: i128) -> i128 {
    core::cmp::max(
        math::sub(
            env,
            physical_cash,
            core::cmp::min(physical_cash, claim_total(env)),
        ),
        0,
    )
}

fn checked_product(env: &Env, a: i128, b: i128) -> i128 {
    a.checked_mul(b)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::ArithmeticError))
}

fn checkpoint_global(env: &Env, now: u64) {
    shared::bump_instance_ttl(env);
    let last = storage::get_u64(env, &storage::Key::LastGlobalCheckpoint);
    if now <= last {
        return;
    }
    let elapsed = (now - last) as i128;

    let rate = storage::get_i128(env, &storage::Key::CurrentBorrowRate);
    let old_rem = storage::get_i128(env, &storage::Key::BorrowIndexRemainder);
    let denominator = BPS * SECONDS_PER_DAY as i128;
    let numerator = math::add(env, checked_product(env, rate, elapsed), old_rem);
    let delta = numerator / denominator;
    storage::set(
        env,
        &storage::Key::BorrowIndex,
        &math::add(
            env,
            storage::get_i128(env, &storage::Key::BorrowIndex),
            delta,
        ),
    );
    storage::set(
        env,
        &storage::Key::BorrowIndexRemainder,
        &(numerator % denominator),
    );

    let flow = storage::get_i128(env, &storage::Key::GlobalReceiverFlow);
    let receiver_num = math::add(
        env,
        checked_product(env, flow, elapsed),
        storage::get_i128(env, &storage::Key::GlobalReceiverRemainder),
    );
    let receiver_delta = receiver_num / INDEX_PRECISION;
    storage::set(
        env,
        &storage::Key::PendingReceiverFundingTotal,
        &math::add(
            env,
            storage::get_i128(env, &storage::Key::PendingReceiverFundingTotal),
            receiver_delta,
        ),
    );
    storage::set(
        env,
        &storage::Key::GlobalReceiverRemainder,
        &(receiver_num % INDEX_PRECISION),
    );
    storage::set(env, &storage::Key::LastGlobalCheckpoint, &now);
}

fn accrue_index(env: &Env, flow: i128, elapsed: i128, base: i128, remainder: i128) -> (i128, i128) {
    if flow == 0 || base == 0 {
        return (0, remainder);
    }
    let numerator = math::add(env, checked_product(env, flow, elapsed), remainder);
    (numerator / base, numerator % base)
}

fn checkpoint_market(env: &Env, market: &mut MarketInfo, now: u64) {
    if now <= market.last_funding_checkpoint {
        return;
    }
    let elapsed = (now - market.last_funding_checkpoint) as i128;
    if market.current_payer_side != 0 {
        let (payer_size, receiver_size) = if market.current_payer_side > 0 {
            (
                market.long.size_open_interest,
                market.short.size_open_interest,
            )
        } else {
            (
                market.short.size_open_interest,
                market.long.size_open_interest,
            )
        };
        let (receiver_delta, receiver_rem) = accrue_index(
            env,
            market.receiver_flow_per_second,
            elapsed,
            payer_size,
            market.receiver_payer_remainder,
        );
        let (lp_delta, lp_rem) = accrue_index(
            env,
            market.current_lp_flow_per_second,
            elapsed,
            payer_size,
            market.lp_payer_remainder,
        );
        let (credit_delta, credit_rem) = accrue_index(
            env,
            market.receiver_flow_per_second,
            elapsed,
            receiver_size,
            market.receiver_index_remainder,
        );
        if market.current_payer_side > 0 {
            market.recv_payer_index_long =
                math::add(env, market.recv_payer_index_long, receiver_delta);
            market.lp_backed_payer_index_long =
                math::add(env, market.lp_backed_payer_index_long, lp_delta);
            market.receiver_index_short = math::add(env, market.receiver_index_short, credit_delta);
        } else {
            market.recv_payer_index_short =
                math::add(env, market.recv_payer_index_short, receiver_delta);
            market.lp_backed_payer_index_short =
                math::add(env, market.lp_backed_payer_index_short, lp_delta);
            market.receiver_index_long = math::add(env, market.receiver_index_long, credit_delta);
        }
        market.receiver_payer_remainder = receiver_rem;
        market.lp_payer_remainder = lp_rem;
        market.receiver_index_remainder = credit_rem;
    }
    market.last_funding_checkpoint = now;
}

fn recompute_market_flow(env: &Env, market: &mut MarketInfo) {
    let old_receiver = market.receiver_flow_per_second;
    let long_base = market.long.base_exposure;
    let short_base = market.short.base_exposure;
    if long_base == short_base || long_base + short_base == 0 {
        market.current_payer_side = 0;
        market.current_payer_rate = 0;
        market.receiver_flow_per_second = 0;
        market.current_lp_flow_per_second = 0;
        if market.long.size_open_interest == 0 && market.short.size_open_interest == 0 {
            market.receiver_payer_remainder = 0;
            market.lp_payer_remainder = 0;
            market.receiver_index_remainder = 0;
            market.receiver_flow_remainder = 0;
        }
    } else {
        let skew = math::skew_bps(env, long_base, short_base);
        let rate = math::funding_rate(env, market.config.max_funding_rate_bps_day, skew);
        let long_pays = long_base > short_base;
        let (dominant_size, dominant_base, light_size, light_base) = if long_pays {
            (
                market.long.size_open_interest,
                long_base,
                market.short.size_open_interest,
                short_base,
            )
        } else {
            (
                market.short.size_open_interest,
                short_base,
                market.long.size_open_interest,
                long_base,
            )
        };
        let payer_flow = math::flow_per_second(env, dominant_size, rate);
        let receiver_flow = if light_size == 0 || light_base == 0 {
            0
        } else {
            let numerator = math::add(
                env,
                checked_product(env, payer_flow, light_base),
                market.receiver_flow_remainder,
            );
            market.receiver_flow_remainder = numerator % dominant_base;
            numerator / dominant_base
        };
        market.current_payer_side = if long_pays { 1 } else { -1 };
        market.current_payer_rate = rate;
        market.receiver_flow_per_second = receiver_flow;
        market.current_lp_flow_per_second = math::sub(env, payer_flow, receiver_flow);
    }
    let global = storage::get_i128(env, &storage::Key::GlobalReceiverFlow);
    storage::set(
        env,
        &storage::Key::GlobalReceiverFlow,
        &math::add(
            env,
            math::sub(env, global, old_receiver),
            market.receiver_flow_per_second,
        ),
    );
}

fn indices_for(market: &MarketInfo, is_long: bool) -> (i128, i128, i128) {
    if is_long {
        (
            market.recv_payer_index_long,
            market.lp_backed_payer_index_long,
            market.receiver_index_long,
        )
    } else {
        (
            market.recv_payer_index_short,
            market.lp_backed_payer_index_short,
            market.receiver_index_short,
        )
    }
}

fn pending_fees(env: &Env, position: &Position, market: &MarketInfo) -> (i128, i128, i128, i128) {
    let (receiver_payer, lp_payer, receiver) = indices_for(market, position.is_long);
    let paid_receiver = math::sub(
        env,
        math::index_value_ceil(env, position.size, receiver_payer),
        position.funding_paid_to_receivers_debt,
    );
    let paid_lp = math::sub(
        env,
        math::index_value_ceil(env, position.size, lp_payer),
        position.funding_paid_to_lps_debt,
    );
    let received = math::sub(
        env,
        math::index_value_floor(env, position.size, receiver),
        position.funding_received_debt,
    );
    let borrow = math::sub(
        env,
        math::index_value_ceil(
            env,
            position.risk_units,
            storage::get_i128(env, &storage::Key::BorrowIndex),
        ),
        position.borrow_debt,
    );
    if paid_receiver < 0 || paid_lp < 0 || received < 0 || borrow < 0 {
        panic_with_error!(env, PositionManagerError::ArithmeticError);
    }
    (paid_receiver, paid_lp, received, borrow)
}

fn reset_debts(env: &Env, position: &mut Position, market: &MarketInfo) {
    let (receiver_payer, lp_payer, receiver) = indices_for(market, position.is_long);
    position.funding_paid_to_receivers_debt =
        math::index_value_ceil(env, position.size, receiver_payer);
    position.funding_paid_to_lps_debt = math::index_value_ceil(env, position.size, lp_payer);
    position.funding_received_debt = math::index_value_floor(env, position.size, receiver);
    position.borrow_debt = math::index_value_ceil(
        env,
        position.risk_units,
        storage::get_i128(env, &storage::Key::BorrowIndex),
    );
}

fn add_position_collateral(env: &Env, position: &mut Position, amount: i128) {
    if amount == 0 {
        return;
    }
    position.collateral = math::add(env, position.collateral, amount);
    storage::set(
        env,
        &storage::Key::StoredCollateralTotal,
        &math::add(
            env,
            storage::get_i128(env, &storage::Key::StoredCollateralTotal),
            amount,
        ),
    );
}

fn collect_position_collateral(env: &Env, position: &mut Position, amount: i128) -> i128 {
    let collected = core::cmp::min(position.collateral, amount);
    position.collateral = math::sub(env, position.collateral, collected);
    storage::set(
        env,
        &storage::Key::StoredCollateralTotal,
        &math::sub(
            env,
            storage::get_i128(env, &storage::Key::StoredCollateralTotal),
            collected,
        ),
    );
    collected
}

fn split_revenue(env: &Env, collected: i128) {
    if collected == 0 {
        return;
    }
    let config = storage::global_config(env);
    let keeper = math::mul_div_floor(
        env,
        collected,
        config.risk_keeper_revenue_share_bps as i128,
        BPS,
    );
    let lp = math::mul_div_floor(env, collected, config.lp_revenue_share_bps as i128, BPS);
    let protocol = math::sub(env, math::sub(env, collected, keeper), lp);
    storage::set(
        env,
        &storage::Key::RiskKeeperReserveTotal,
        &math::add(
            env,
            storage::get_i128(env, &storage::Key::RiskKeeperReserveTotal),
            keeper,
        ),
    );
    storage::set(
        env,
        &storage::Key::ProtocolClaimableTotal,
        &math::add(
            env,
            storage::get_i128(env, &storage::Key::ProtocolClaimableTotal),
            protocol,
        ),
    );
}

/// Capitalize all accrued amounts. Negative PnL is collected after guaranteed
/// receiver funding and before LP-backed funding and borrow.
fn capitalize(env: &Env, position: &mut Position, market: &MarketInfo, negative_pnl: i128) -> i128 {
    let (receiver_due, lp_due, receiver_credit_raw, borrow_due) =
        pending_fees(env, position, market);
    let receiver_credit = core::cmp::min(
        receiver_credit_raw,
        storage::get_i128(env, &storage::Key::PendingReceiverFundingTotal),
    );
    if receiver_credit > 0 {
        storage::set(
            env,
            &storage::Key::PendingReceiverFundingTotal,
            &math::sub(
                env,
                storage::get_i128(env, &storage::Key::PendingReceiverFundingTotal),
                receiver_credit,
            ),
        );
        add_position_collateral(env, position, receiver_credit);
    }

    let receiver_collected = collect_position_collateral(env, position, receiver_due);
    let negative_collected = collect_position_collateral(env, position, negative_pnl);
    let lp_collected = collect_position_collateral(env, position, lp_due);
    let borrow_collected = collect_position_collateral(env, position, borrow_due);
    split_revenue(env, borrow_collected);
    reset_debts(env, position, market);

    let guaranteed_and_loss = math::add(
        env,
        math::sub(env, receiver_due, receiver_collected),
        math::sub(env, negative_pnl, negative_collected),
    );
    math::add(
        env,
        math::add(
            env,
            guaranteed_and_loss,
            math::sub(env, lp_due, lp_collected),
        ),
        math::sub(env, borrow_due, borrow_collected),
    )
}

fn apply_opening_fee(env: &Env, position: &mut Position, fee: i128) {
    let collected = collect_position_collateral(env, position, fee);
    if collected != fee {
        panic_with_error!(env, PositionManagerError::InsufficientCollateral);
    }
    split_revenue(env, collected);
}

fn side_mut(market: &mut MarketInfo, is_long: bool) -> &mut MarketSide {
    if is_long {
        &mut market.long
    } else {
        &mut market.short
    }
}

fn check_slippage(env: &Env, is_long: bool, opening: bool, price: i128, acceptable: i128) {
    if acceptable == 0 {
        return;
    }
    let bad = if opening {
        if is_long {
            price > acceptable
        } else {
            price < acceptable
        }
    } else if is_long {
        price < acceptable
    } else {
        price > acceptable
    };
    if bad {
        panic_with_error!(env, PositionManagerError::SlippageExceeded);
    }
}

fn validate_orders(env: &Env, is_long: bool, take_profit: i128, stop_loss: i128, price: i128) {
    if take_profit < 0 || stop_loss < 0 {
        panic_with_error!(env, PositionManagerError::InvalidOrder);
    }
    let invalid = if is_long {
        (take_profit > 0 && take_profit <= price) || (stop_loss > 0 && stop_loss >= price)
    } else {
        (take_profit > 0 && take_profit >= price) || (stop_loss > 0 && stop_loss <= price)
    };
    if invalid {
        panic_with_error!(env, PositionManagerError::InvalidOrder);
    }
}

fn physical_cash(env: &Env) -> i128 {
    VaultClient::new(env, &storage::vault(env)).physical_cash()
}

fn refresh_rate(env: &Env, physical: i128) {
    let config = storage::global_config(env);
    let utilization = math::utilization_bps(
        env,
        storage::get_i128(env, &storage::Key::TotalRiskUnits),
        cash_equity(env, physical),
    );
    storage::set(
        env,
        &storage::Key::CurrentBorrowRate,
        &math::borrow_rate(
            env,
            config.base_borrow_rate_bps_day,
            config.max_variable_borrow_bps_day,
            utilization,
        ),
    );
}

fn enforce_capacity(env: &Env, physical: i128, risk_after: i128) {
    let config = storage::global_config(env);
    let limit = math::mul_div_floor(
        env,
        cash_equity(env, physical),
        config.risk_capacity_limit_bps as i128,
        BPS,
    );
    if risk_after > limit {
        panic_with_error!(env, PositionManagerError::CapacityExceeded);
    }
}

fn enforce_market_limits(env: &Env, market: &MarketInfo, is_long: bool) {
    let side = if is_long { &market.long } else { &market.short };
    let (size_cap, base_cap) = if is_long {
        (
            market.config.max_long_size_open_interest,
            market.config.max_long_base_exposure,
        )
    } else {
        (
            market.config.max_short_size_open_interest,
            market.config.max_short_base_exposure,
        )
    };
    if side.size_open_interest > size_cap || side.base_exposure > base_cap {
        panic_with_error!(env, PositionManagerError::MarketLimitExceeded);
    }
}

fn maintenance_requirement(env: &Env, size: i128, config: &MarketConfig) -> i128 {
    math::mul_div_ceil(env, size, config.maintenance_margin_bps as i128, BPS)
}

fn round_price(env: &Env, round: &OracleRound, market: &Symbol, index: u32) -> i128 {
    let item = round
        .prices
        .get(index)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::InvalidOracleRound));
    if item.symbol != *market || item.price <= 0 {
        panic_with_error!(env, PositionManagerError::InvalidOracleRound);
    }
    item.price
}

fn risk_state_for(
    env: &Env,
    current: RiskState,
    positive_pnl: i128,
    equity: i128,
    config: &MarketConfig,
) -> RiskState {
    let factor = if positive_pnl == 0 {
        0
    } else if equity == 0 {
        BPS
    } else {
        math::mul_div_floor(env, positive_pnl, BPS, equity)
    };
    if factor >= config.hard_cap_pnl_factor_bps as i128 {
        RiskState::HardCap
    } else if factor >= config.adl_pnl_factor_bps as i128 {
        RiskState::Adl
    } else if factor >= config.warning_pnl_factor_bps as i128 {
        RiskState::Warning
    } else if current != RiskState::Normal && factor >= config.recovery_pnl_factor_bps as i128 {
        RiskState::Warning
    } else {
        RiskState::Normal
    }
}

fn update_blocked_count(env: &Env, old: RiskState, new: RiskState) {
    if old == RiskState::Normal && new != RiskState::Normal {
        storage::set(
            env,
            &storage::Key::LpBlockedSideCount,
            &(storage::get_u32(env, &storage::Key::LpBlockedSideCount) + 1),
        );
    } else if old != RiskState::Normal && new == RiskState::Normal {
        storage::set(
            env,
            &storage::Key::LpBlockedSideCount,
            &storage::get_u32(env, &storage::Key::LpBlockedSideCount).saturating_sub(1),
        );
    }
}

fn evaluate_market_risk(env: &Env, market: &mut MarketInfo, price: i128, equity: i128) {
    let long_pnl = core::cmp::max(
        math::pnl(
            env,
            true,
            market.long.size_open_interest,
            market.long.base_exposure,
            price,
        ),
        0,
    );
    let short_pnl = core::cmp::max(
        math::pnl(
            env,
            false,
            market.short.size_open_interest,
            market.short.base_exposure,
            price,
        ),
        0,
    );
    let long_new = risk_state_for(
        env,
        market.long.risk_state,
        long_pnl,
        equity,
        &market.config,
    );
    let short_new = risk_state_for(
        env,
        market.short.risk_state,
        short_pnl,
        equity,
        &market.config,
    );
    update_blocked_count(env, market.long.risk_state, long_new);
    update_blocked_count(env, market.short.risk_state, short_new);
    market.long.risk_state = long_new;
    market.short.risk_state = short_new;
}

fn build_snapshot(
    env: &Env,
    round: &OracleRound,
    physical: i128,
    mutate_risk: bool,
) -> AccountingSnapshot {
    let markets = storage::active_markets(env);
    if round.prices.len() != markets.len() {
        panic_with_error!(env, PositionManagerError::InvalidOracleRound);
    }
    let claims = claim_total(env);
    let shortfall = core::cmp::max(math::sub(env, claims, physical), 0);
    let equity = core::cmp::max(
        math::sub(env, physical, core::cmp::min(physical, claims)),
        0,
    );
    let mut aggregate_pnl_numerator = 0i128;
    let mut blocked_side_count = 0u32;

    let mut i = 0u32;
    while i < markets.len() {
        let symbol = markets.get(i).unwrap();
        let price = round_price(env, round, &symbol, i);
        let mut market = storage::market(env, &symbol)
            .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::MarketNotConfigured));
        let long_num = checked_product(env, market.long.base_exposure, price)
            - checked_product(env, market.long.size_open_interest, PRECISION);
        let short_num = checked_product(env, market.short.size_open_interest, PRECISION)
            - checked_product(env, market.short.base_exposure, price);
        let long_recognized = if long_num >= 0 {
            long_num
        } else {
            -core::cmp::min(
                long_num.abs(),
                checked_product(env, market.long.stored_collateral_total, PRECISION),
            )
        };
        let short_recognized = if short_num >= 0 {
            short_num
        } else {
            -core::cmp::min(
                short_num.abs(),
                checked_product(env, market.short.stored_collateral_total, PRECISION),
            )
        };
        aggregate_pnl_numerator = math::add(
            env,
            aggregate_pnl_numerator,
            math::add(env, long_recognized, short_recognized),
        );
        if mutate_risk {
            evaluate_market_risk(env, &mut market, price, equity);
            storage::save_market(env, &symbol, &market);
        } else {
            market.long.risk_state = risk_state_for(
                env,
                market.long.risk_state,
                core::cmp::max(
                    math::pnl(
                        env,
                        true,
                        market.long.size_open_interest,
                        market.long.base_exposure,
                        price,
                    ),
                    0,
                ),
                equity,
                &market.config,
            );
            market.short.risk_state = risk_state_for(
                env,
                market.short.risk_state,
                core::cmp::max(
                    math::pnl(
                        env,
                        false,
                        market.short.size_open_interest,
                        market.short.base_exposure,
                        price,
                    ),
                    0,
                ),
                equity,
                &market.config,
            );
        }
        if market.long.risk_state != RiskState::Normal {
            blocked_side_count += 1;
        }
        if market.short.risk_state != RiskState::Normal {
            blocked_side_count += 1;
        }
        i += 1;
    }
    let nav_num = math::sub(
        env,
        checked_product(env, equity, PRECISION),
        aggregate_pnl_numerator,
    );
    let nav = if nav_num <= 0 { 0 } else { nav_num / PRECISION };
    let risk = storage::get_i128(env, &storage::Key::TotalRiskUnits);
    let config = storage::global_config(env);
    let required = if risk == 0 {
        0
    } else {
        math::mul_div_ceil(env, risk, BPS, config.risk_capacity_limit_bps as i128)
    };
    AccountingSnapshot {
        physical_cash: physical,
        non_lp_claims: claims,
        cash_lp_equity: equity,
        cash_shortfall: shortfall,
        required_risk_backing: required,
        free_lp_capital: core::cmp::max(
            math::sub(env, equity, core::cmp::min(equity, required)),
            0,
        ),
        vault_nav: nav,
        total_risk_units: risk,
        open_position_count: storage::get_u64(env, &storage::Key::OpenPositionCount),
        lp_blocked_side_count: blocked_side_count,
    }
}

fn authenticated_price(env: &Env, symbol: &Symbol) -> i128 {
    let price = OracleRouterClient::new(env, &storage::oracle_router(env)).get_price(symbol);
    if price <= 0 {
        panic_with_error!(env, PositionManagerError::InvalidOracleRound);
    }
    price
}

fn payable_price_pnl(
    env: &Env,
    position: &Position,
    market: &MarketInfo,
    size: i128,
    base: i128,
    price: i128,
) -> i128 {
    let raw = math::pnl(env, position.is_long, size, base, price);
    if raw <= 0 {
        return raw;
    }
    let side = if position.is_long {
        &market.long
    } else {
        &market.short
    };
    if side.risk_state != RiskState::HardCap {
        return raw;
    }
    let side_positive = core::cmp::max(
        math::pnl(
            env,
            position.is_long,
            side.size_open_interest,
            side.base_exposure,
            price,
        ),
        0,
    );
    if side_positive == 0 {
        return 0;
    }
    let hard_cap_value = math::mul_div_floor(
        env,
        cash_equity(env, physical_cash(env)),
        market.config.hard_cap_pnl_factor_bps as i128,
        BPS,
    );
    math::mul_div_floor(
        env,
        raw,
        core::cmp::min(hard_cap_value, side_positive),
        side_positive,
    )
}

fn settle_close(
    env: &Env,
    mut position: Position,
    mut market: MarketInfo,
    size_removed: i128,
    collateral_withdrawn: i128,
    price: i128,
    reward_recipient: Option<&Address>,
) -> bool {
    let original_collateral = position.collateral;
    evaluate_market_risk(
        env,
        &mut market,
        price,
        cash_equity(env, physical_cash(env)),
    );
    let full = size_removed == position.size;
    let new_size = math::sub(env, position.size, size_removed);
    let base_after = math::remaining(env, position.base_exposure, position.size, new_size);
    let risk_after = math::remaining(env, position.risk_units, position.size, new_size);
    let base_removed = math::sub(env, position.base_exposure, base_after);
    let risk_removed = math::sub(env, position.risk_units, risk_after);
    let raw_pnl = math::pnl(env, position.is_long, size_removed, base_removed, price);
    let positive = core::cmp::max(
        payable_price_pnl(env, &position, &market, size_removed, base_removed, price),
        0,
    );
    let negative = core::cmp::max(-raw_pnl, 0);
    if positive > 0 {
        add_position_collateral(env, &mut position, positive);
    }
    let unpaid = capitalize(env, &mut position, &market, negative);

    if unpaid > 0 && !full {
        panic_with_error!(env, PositionManagerError::InsufficientCollateral);
    }

    if !full && positive > 0 {
        let realized_payout = core::cmp::min(positive, position.collateral);
        if realized_payout > 0 {
            collect_position_collateral(env, &mut position, realized_payout);
            VaultClient::new(env, &storage::vault(env)).transfer_safety_claim(
                &env.current_contract_address(),
                &position.owner,
                &realized_payout,
            );
        }
    }

    let side = side_mut(&mut market, position.is_long);
    side.size_open_interest = math::sub(env, side.size_open_interest, size_removed);
    side.base_exposure = math::sub(env, side.base_exposure, base_removed);
    side.risk_units = math::sub(env, side.risk_units, risk_removed);
    side.stored_collateral_total = math::add(
        env,
        math::sub(env, side.stored_collateral_total, original_collateral),
        position.collateral,
    );
    storage::set(
        env,
        &storage::Key::TotalRiskUnits,
        &math::sub(
            env,
            storage::get_i128(env, &storage::Key::TotalRiskUnits),
            risk_removed,
        ),
    );

    let must_close = full;
    if must_close {
        if unpaid > 0 {
            events::BadDebt {
                position_id: position.id,
                amount: unpaid,
            }
            .publish(env);
        }
        if let Some(liquidator) = reward_recipient {
            let reward = core::cmp::min(
                position.collateral,
                math::mul_div_floor(
                    env,
                    size_removed,
                    market.config.liquidation_reward_bps as i128,
                    BPS,
                ),
            );
            if reward > 0 {
                collect_position_collateral(env, &mut position, reward);
                let reward_side = side_mut(&mut market, position.is_long);
                reward_side.stored_collateral_total =
                    math::sub(env, reward_side.stored_collateral_total, reward);
                VaultClient::new(env, &storage::vault(env)).transfer_safety_claim(
                    &env.current_contract_address(),
                    liquidator,
                    &reward,
                );
            }
        }
        let payout = position.collateral;
        if payout > 0 {
            collect_position_collateral(env, &mut position, payout);
            let payout_side = side_mut(&mut market, position.is_long);
            payout_side.stored_collateral_total =
                math::sub(env, payout_side.stored_collateral_total, payout);
            VaultClient::new(env, &storage::vault(env)).transfer_safety_claim(
                &env.current_contract_address(),
                &position.owner,
                &payout,
            );
        }
        if position.execution_budget > 0 {
            let budget = position.execution_budget;
            storage::set(
                env,
                &storage::Key::ExecutionBudgetTotal,
                &math::sub(
                    env,
                    storage::get_i128(env, &storage::Key::ExecutionBudgetTotal),
                    budget,
                ),
            );
            VaultClient::new(env, &storage::vault(env)).transfer_safety_claim(
                &env.current_contract_address(),
                &position.owner,
                &budget,
            );
        }
        storage::remove_position(env, position.id);
        storage::set(
            env,
            &storage::Key::OpenPositionCount,
            &storage::get_u64(env, &storage::Key::OpenPositionCount).saturating_sub(1),
        );
    } else {
        if collateral_withdrawn > position.collateral {
            panic_with_error!(env, PositionManagerError::InsufficientCollateral);
        }
        if collateral_withdrawn > 0 {
            collect_position_collateral(env, &mut position, collateral_withdrawn);
            let withdrawal_side = side_mut(&mut market, position.is_long);
            withdrawal_side.stored_collateral_total = math::sub(
                env,
                withdrawal_side.stored_collateral_total,
                collateral_withdrawn,
            );
            let vault_client = VaultClient::new(env, &storage::vault(env));
            if size_removed > 0 {
                vault_client.transfer_safety_claim(
                    &env.current_contract_address(),
                    &position.owner,
                    &collateral_withdrawn,
                );
            } else {
                vault_client.transfer_claim(
                    &env.current_contract_address(),
                    &position.owner,
                    &collateral_withdrawn,
                    &claim_total(env),
                );
            }
        }
        position.size = new_size;
        position.base_exposure = base_after;
        position.risk_units = risk_after;
        let health = math::add(
            env,
            position.collateral,
            math::pnl(env, position.is_long, new_size, base_after, price),
        );
        if health < maintenance_requirement(env, new_size, &market.config) {
            panic_with_error!(env, PositionManagerError::InsufficientCollateral);
        }
        reset_debts(env, &mut position, &market);
        storage::save_position(env, &position);
    }
    recompute_market_flow(env, &mut market);
    evaluate_market_risk(
        env,
        &mut market,
        price,
        cash_equity(env, physical_cash(env)),
    );
    if storage::get_u64(env, &storage::Key::OpenPositionCount) == 0
        && storage::get_i128(env, &storage::Key::GlobalReceiverFlow) == 0
    {
        storage::set(env, &storage::Key::PendingReceiverFundingTotal, &0i128);
        storage::set(env, &storage::Key::GlobalReceiverRemainder, &0i128);
    }
    storage::save_market(env, &position.market, &market);
    refresh_rate(env, physical_cash(env));
    must_close
}

#[contractimpl]
impl PositionManagerContract {
    pub fn __constructor(
        env: Env,
        config_manager: Address,
        oracle_router: Address,
        config: GlobalConfig,
    ) {
        validate_global(&env, &config);
        storage::set(&env, &storage::Key::ConfigManager, &config_manager);
        storage::set(&env, &storage::Key::OracleRouter, &oracle_router);
        storage::set(&env, &storage::Key::GlobalConfig, &config);
        storage::set(&env, &storage::Key::Initialized, &true);
        storage::set(&env, &storage::Key::Paused, &false);
        storage::set(&env, &storage::Key::NextPositionId, &1u64);
        storage::set(
            &env,
            &storage::Key::ActiveMarkets,
            &Vec::<Symbol>::new(&env),
        );
        storage::set(
            &env,
            &storage::Key::LastGlobalCheckpoint,
            &env.ledger().timestamp(),
        );
        storage::set(
            &env,
            &storage::Key::CurrentBorrowRate,
            &checked_product(&env, config.base_borrow_rate_bps_day, INDEX_PRECISION),
        );
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl PositionManager for PositionManagerContract {
    fn set_vault(env: Env, caller: Address, vault: Address) {
        require_initialized(&env);
        require_role(&env, &caller, ROLE_ADMIN);
        if storage::get::<Address>(&env, &storage::Key::Vault).is_some() {
            panic_with_error!(&env, PositionManagerError::AlreadyInitialized);
        }
        storage::set(&env, &storage::Key::Vault, &vault);
    }

    fn open_position(
        env: Env,
        owner: Address,
        market_symbol: Symbol,
        is_long: bool,
        size: i128,
        collateral: i128,
        execution_budget: i128,
        take_profit: i128,
        stop_loss: i128,
        acceptable_price: i128,
    ) -> u64 {
        require_initialized(&env);
        owner.require_auth();
        if storage::get::<bool>(&env, &storage::Key::Paused) == Some(true) {
            panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
        }
        if size <= 0 || collateral <= 0 || execution_budget < 0 {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        if storage::get::<bool>(&env, &storage::Key::MarketDisabled(market_symbol.clone()))
            == Some(true)
        {
            panic_with_error!(&env, PositionManagerError::MarketDisabled);
        }
        let mut market = storage::market(&env, &market_symbol)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::MarketNotConfigured));
        let now = env.ledger().timestamp();
        checkpoint_global(&env, now);
        checkpoint_market(&env, &mut market, now);
        let price = authenticated_price(&env, &market_symbol);
        check_slippage(&env, is_long, true, price, acceptable_price);
        validate_orders(&env, is_long, take_profit, stop_loss, price);

        let total_transfer = math::add(&env, collateral, execution_budget);
        VaultClient::new(&env, &storage::vault(&env)).receive_collateral(
            &env.current_contract_address(),
            &owner,
            &total_transfer,
        );
        storage::set(
            &env,
            &storage::Key::ExecutionBudgetTotal,
            &math::add(
                &env,
                storage::get_i128(&env, &storage::Key::ExecutionBudgetTotal),
                execution_budget,
            ),
        );
        let id = storage::get_u64(&env, &storage::Key::NextPositionId);
        storage::set(&env, &storage::Key::NextPositionId, &(id + 1));
        let base = math::base_added(&env, size, price);
        let risk = math::risk_added(&env, size, market.config.market_risk_factor_bps);
        let mut position = Position {
            id,
            owner: owner.clone(),
            market: market_symbol.clone(),
            is_long,
            size,
            base_exposure: base,
            collateral: 0,
            risk_units: risk,
            borrow_debt: 0,
            funding_paid_to_receivers_debt: 0,
            funding_paid_to_lps_debt: 0,
            funding_received_debt: 0,
            execution_budget,
            last_increased_time: now,
            take_profit,
            stop_loss,
        };
        add_position_collateral(&env, &mut position, collateral);
        let old_skew = math::skew_bps(&env, market.long.base_exposure, market.short.base_exposure);
        let (long_after, short_after) = if is_long {
            (
                math::add(&env, market.long.base_exposure, base),
                market.short.base_exposure,
            )
        } else {
            (
                market.long.base_exposure,
                math::add(&env, market.short.base_exposure, base),
            )
        };
        let new_skew = math::skew_bps(&env, long_after, short_after);
        let fee_bps = if new_skew <= old_skew {
            market.config.open_fee_low_bps
        } else {
            market.config.open_fee_high_bps
        };
        apply_opening_fee(&env, &mut position, math::opening_fee(&env, size, fee_bps));
        if position.collateral < storage::global_config(&env).min_collateral
            || position.collateral < maintenance_requirement(&env, size, &market.config)
        {
            panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
        }
        evaluate_market_risk(
            &env,
            &mut market,
            price,
            cash_equity(&env, physical_cash(&env)),
        );
        if side_mut(&mut market, is_long).risk_state != RiskState::Normal {
            panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
        }
        {
            let side = side_mut(&mut market, is_long);
            side.size_open_interest = math::add(&env, side.size_open_interest, size);
            side.base_exposure = math::add(&env, side.base_exposure, base);
            side.risk_units = math::add(&env, side.risk_units, risk);
            side.stored_collateral_total =
                math::add(&env, side.stored_collateral_total, position.collateral);
        }
        let risk_after = math::add(
            &env,
            storage::get_i128(&env, &storage::Key::TotalRiskUnits),
            risk,
        );
        storage::set(&env, &storage::Key::TotalRiskUnits, &risk_after);
        enforce_capacity(&env, physical_cash(&env), risk_after);
        enforce_market_limits(&env, &market, is_long);
        reset_debts(&env, &mut position, &market);
        storage::save_position(&env, &position);
        storage::set(
            &env,
            &storage::Key::OpenPositionCount,
            &(storage::get_u64(&env, &storage::Key::OpenPositionCount) + 1),
        );
        recompute_market_flow(&env, &mut market);
        storage::save_market(&env, &market_symbol, &market);
        refresh_rate(&env, physical_cash(&env));
        events::PositionOpened {
            position_id: id,
            owner,
            market: market_symbol,
        }
        .publish(&env);
        id
    }

    fn increase_position(
        env: Env,
        position_id: u64,
        size_added: i128,
        collateral_added: i128,
        acceptable_price: i128,
    ) {
        require_initialized(&env);
        if size_added < 0 || collateral_added < 0 || (size_added == 0 && collateral_added == 0) {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        let mut position = storage::position(&env, position_id)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound));
        let original_collateral = position.collateral;
        position.owner.require_auth();
        if storage::get::<bool>(&env, &storage::Key::Paused) == Some(true)
            || storage::get::<bool>(&env, &storage::Key::MarketDisabled(position.market.clone()))
                == Some(true)
        {
            panic_with_error!(&env, PositionManagerError::MarketDisabled);
        }
        let mut market = storage::market(&env, &position.market).unwrap();
        let now = env.ledger().timestamp();
        checkpoint_global(&env, now);
        checkpoint_market(&env, &mut market, now);
        let price = authenticated_price(&env, &position.market);
        check_slippage(&env, position.is_long, true, price, acceptable_price);
        if collateral_added > 0 {
            VaultClient::new(&env, &storage::vault(&env)).receive_collateral(
                &env.current_contract_address(),
                &position.owner,
                &collateral_added,
            );
            add_position_collateral(&env, &mut position, collateral_added);
        }
        if capitalize(&env, &mut position, &market, 0) > 0 {
            panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
        }
        let base = math::base_added(&env, size_added, price);
        let risk = math::risk_added(&env, size_added, market.config.market_risk_factor_bps);
        let old_skew = math::skew_bps(&env, market.long.base_exposure, market.short.base_exposure);
        let (long_after, short_after) = if position.is_long {
            (
                math::add(&env, market.long.base_exposure, base),
                market.short.base_exposure,
            )
        } else {
            (
                market.long.base_exposure,
                math::add(&env, market.short.base_exposure, base),
            )
        };
        let fee_bps = if math::skew_bps(&env, long_after, short_after) <= old_skew {
            market.config.open_fee_low_bps
        } else {
            market.config.open_fee_high_bps
        };
        apply_opening_fee(
            &env,
            &mut position,
            math::opening_fee(&env, size_added, fee_bps),
        );
        evaluate_market_risk(
            &env,
            &mut market,
            price,
            cash_equity(&env, physical_cash(&env)),
        );
        if size_added > 0 && side_mut(&mut market, position.is_long).risk_state != RiskState::Normal
        {
            panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
        }
        position.size = math::add(&env, position.size, size_added);
        position.base_exposure = math::add(&env, position.base_exposure, base);
        position.risk_units = math::add(&env, position.risk_units, risk);
        if size_added > 0 {
            position.last_increased_time = now;
        }
        {
            let side = side_mut(&mut market, position.is_long);
            side.size_open_interest = math::add(&env, side.size_open_interest, size_added);
            side.base_exposure = math::add(&env, side.base_exposure, base);
            side.risk_units = math::add(&env, side.risk_units, risk);
            side.stored_collateral_total = math::add(
                &env,
                math::sub(&env, side.stored_collateral_total, original_collateral),
                position.collateral,
            );
        }
        let risk_after = math::add(
            &env,
            storage::get_i128(&env, &storage::Key::TotalRiskUnits),
            risk,
        );
        storage::set(&env, &storage::Key::TotalRiskUnits, &risk_after);
        enforce_capacity(&env, physical_cash(&env), risk_after);
        enforce_market_limits(&env, &market, position.is_long);
        let health = math::add(
            &env,
            position.collateral,
            math::pnl(
                &env,
                position.is_long,
                position.size,
                position.base_exposure,
                price,
            ),
        );
        if health < maintenance_requirement(&env, position.size, &market.config) {
            panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
        }
        reset_debts(&env, &mut position, &market);
        storage::save_position(&env, &position);
        recompute_market_flow(&env, &mut market);
        storage::save_market(&env, &position.market, &market);
        refresh_rate(&env, physical_cash(&env));
    }

    fn decrease_position(
        env: Env,
        position_id: u64,
        size_removed: i128,
        collateral_withdrawn: i128,
        acceptable_price: i128,
    ) {
        require_initialized(&env);
        let position = storage::position(&env, position_id)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound));
        position.owner.require_auth();
        if size_removed < 0
            || size_removed > position.size
            || collateral_withdrawn < 0
            || (size_removed == 0 && collateral_withdrawn == 0)
        {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        let config = storage::global_config(&env);
        if env.ledger().timestamp()
            < position
                .last_increased_time
                .saturating_add(config.min_position_lifetime)
        {
            panic_with_error!(&env, PositionManagerError::TooEarly);
        }
        let mut market = storage::market(&env, &position.market).unwrap();
        let now = env.ledger().timestamp();
        checkpoint_global(&env, now);
        checkpoint_market(&env, &mut market, now);
        let price = authenticated_price(&env, &position.market);
        check_slippage(&env, position.is_long, false, price, acceptable_price);
        settle_close(
            &env,
            position,
            market,
            size_removed,
            collateral_withdrawn,
            price,
            None,
        );
    }

    fn liquidate_position(env: Env, caller: Address, position_id: u64) {
        require_initialized(&env);
        caller.require_auth();
        let position = storage::position(&env, position_id)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound));
        let mut market = storage::market(&env, &position.market).unwrap();
        let now = env.ledger().timestamp();
        checkpoint_global(&env, now);
        checkpoint_market(&env, &mut market, now);
        let price = authenticated_price(&env, &position.market);
        evaluate_market_risk(
            &env,
            &mut market,
            price,
            cash_equity(&env, physical_cash(&env)),
        );
        let (receiver, lp, credit, borrow) = pending_fees(&env, &position, &market);
        let effective = position.collateral + credit - receiver - lp - borrow
            + payable_price_pnl(
                &env,
                &position,
                &market,
                position.size,
                position.base_exposure,
                price,
            );
        if effective >= maintenance_requirement(&env, position.size, &market.config) {
            panic_with_error!(&env, PositionManagerError::PositionHealthy);
        }
        let insolvent = effective < 0;
        let size = position.size;
        let closed = settle_close(&env, position, market, size, 0, price, Some(&caller));
        if closed && insolvent {
            let reserve = storage::get_i128(&env, &storage::Key::RiskKeeperReserveTotal);
            let reward = core::cmp::min(
                reserve,
                storage::global_config(&env).max_insolvent_touch_reward,
            );
            if reward > 0 {
                storage::set(
                    &env,
                    &storage::Key::RiskKeeperReserveTotal,
                    &math::sub(&env, reserve, reward),
                );
                VaultClient::new(&env, &storage::vault(&env)).transfer_safety_claim(
                    &env.current_contract_address(),
                    &caller,
                    &reward,
                );
            }
        }
    }

    fn deleverage_position(env: Env, caller: Address, position_id: u64) {
        require_initialized(&env);
        require_role(&env, &caller, ROLE_KEEPER);
        let position = storage::position(&env, position_id)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound));
        let mut market = storage::market(&env, &position.market).unwrap();
        let now = env.ledger().timestamp();
        checkpoint_global(&env, now);
        checkpoint_market(&env, &mut market, now);
        let price = authenticated_price(&env, &position.market);
        evaluate_market_risk(
            &env,
            &mut market,
            price,
            cash_equity(&env, physical_cash(&env)),
        );
        let side = if position.is_long {
            &market.long
        } else {
            &market.short
        };
        if side.risk_state != RiskState::Adl && side.risk_state != RiskState::HardCap {
            panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
        }
        if math::pnl(
            &env,
            position.is_long,
            position.size,
            position.base_exposure,
            price,
        ) <= 0
        {
            panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
        }
        let size = position.size;
        let reward_bps = market.config.adl_reward_bps;
        settle_close(&env, position, market, size, 0, price, None);
        let reserve = storage::get_i128(&env, &storage::Key::RiskKeeperReserveTotal);
        let configured_reward = math::mul_div_floor(&env, size, reward_bps as i128, BPS);
        let reward = core::cmp::min(
            core::cmp::min(reserve, storage::global_config(&env).max_adl_reward),
            configured_reward,
        );
        if reward > 0 {
            storage::set(
                &env,
                &storage::Key::RiskKeeperReserveTotal,
                &math::sub(&env, reserve, reward),
            );
            VaultClient::new(&env, &storage::vault(&env)).transfer_safety_claim(
                &env.current_contract_address(),
                &caller,
                &reward,
            );
        }
    }

    fn execute_order(env: Env, caller: Address, position_id: u64) {
        require_initialized(&env);
        caller.require_auth();
        let mut position = storage::position(&env, position_id)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound));
        let price = authenticated_price(&env, &position.market);
        let triggered = if position.is_long {
            (position.take_profit > 0 && price >= position.take_profit)
                || (position.stop_loss > 0 && price <= position.stop_loss)
        } else {
            (position.take_profit > 0 && price <= position.take_profit)
                || (position.stop_loss > 0 && price >= position.stop_loss)
        };
        if !triggered {
            panic_with_error!(&env, PositionManagerError::InvalidOrder);
        }
        if position.execution_budget <= 0 {
            panic_with_error!(&env, PositionManagerError::InsufficientExecutionBudget);
        }
        let budget = position.execution_budget;
        position.execution_budget = 0;
        storage::save_position(&env, &position);
        storage::set(
            &env,
            &storage::Key::ExecutionBudgetTotal,
            &math::sub(
                &env,
                storage::get_i128(&env, &storage::Key::ExecutionBudgetTotal),
                budget,
            ),
        );
        VaultClient::new(&env, &storage::vault(&env)).transfer_safety_claim(
            &env.current_contract_address(),
            &caller,
            &budget,
        );
        let mut market = storage::market(&env, &position.market).unwrap();
        let now = env.ledger().timestamp();
        checkpoint_global(&env, now);
        checkpoint_market(&env, &mut market, now);
        let size = position.size;
        settle_close(&env, position, market, size, 0, price, None);
    }

    fn set_tp_sl(env: Env, position_id: u64, take_profit: i128, stop_loss: i128) {
        let mut position = storage::position(&env, position_id)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound));
        position.owner.require_auth();
        let price = authenticated_price(&env, &position.market);
        validate_orders(&env, position.is_long, take_profit, stop_loss, price);
        position.take_profit = take_profit;
        position.stop_loss = stop_loss;
        storage::save_position(&env, &position);
    }

    fn fund_execution_budget(env: Env, position_id: u64, amount: i128) {
        if amount <= 0 {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        let mut position = storage::position(&env, position_id)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound));
        position.owner.require_auth();
        checkpoint_global(&env, env.ledger().timestamp());
        VaultClient::new(&env, &storage::vault(&env)).receive_collateral(
            &env.current_contract_address(),
            &position.owner,
            &amount,
        );
        position.execution_budget = math::add(&env, position.execution_budget, amount);
        storage::set(
            &env,
            &storage::Key::ExecutionBudgetTotal,
            &math::add(
                &env,
                storage::get_i128(&env, &storage::Key::ExecutionBudgetTotal),
                amount,
            ),
        );
        storage::save_position(&env, &position);
        refresh_rate(&env, physical_cash(&env));
    }

    fn withdraw_execution_budget(env: Env, position_id: u64, amount: i128) {
        if amount <= 0 {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        let mut position = storage::position(&env, position_id)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound));
        position.owner.require_auth();
        checkpoint_global(&env, env.ledger().timestamp());
        if amount > position.execution_budget {
            panic_with_error!(&env, PositionManagerError::InsufficientExecutionBudget);
        }
        position.execution_budget = math::sub(&env, position.execution_budget, amount);
        storage::set(
            &env,
            &storage::Key::ExecutionBudgetTotal,
            &math::sub(
                &env,
                storage::get_i128(&env, &storage::Key::ExecutionBudgetTotal),
                amount,
            ),
        );
        storage::save_position(&env, &position);
        VaultClient::new(&env, &storage::vault(&env)).transfer_claim(
            &env.current_contract_address(),
            &position.owner,
            &amount,
            &claim_total(&env),
        );
        refresh_rate(&env, physical_cash(&env));
    }

    fn update_indices(env: Env, caller: Address, market_symbol: Symbol) {
        require_role(&env, &caller, ROLE_KEEPER);
        let now = env.ledger().timestamp();
        checkpoint_global(&env, now);
        let mut market = storage::market(&env, &market_symbol)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::MarketNotConfigured));
        checkpoint_market(&env, &mut market, now);
        storage::save_market(&env, &market_symbol, &market);
        refresh_rate(&env, physical_cash(&env));
    }

    fn set_global_config(env: Env, caller: Address, config: GlobalConfig) {
        require_role(&env, &caller, ROLE_ADMIN);
        validate_global(&env, &config);
        checkpoint_global(&env, env.ledger().timestamp());
        let markets = storage::active_markets(&env);
        if markets.len() > config.max_active_markets {
            panic_with_error!(&env, PositionManagerError::InvalidConfig);
        }
        let mut hard_sum = 0u64;
        let mut i = 0u32;
        while i < markets.len() {
            let symbol = markets.get(i).unwrap();
            hard_sum += storage::market(&env, &symbol)
                .unwrap()
                .config
                .hard_cap_pnl_factor_bps as u64
                * 2;
            i += 1;
        }
        if hard_sum > config.hard_cap_factor_limit_bps as u64 {
            panic_with_error!(&env, PositionManagerError::InvalidConfig);
        }
        storage::set(&env, &storage::Key::GlobalConfig, &config);
        refresh_rate(&env, physical_cash(&env));
    }

    fn set_market_config(env: Env, caller: Address, market_symbol: Symbol, config: MarketConfig) {
        require_role(&env, &caller, ROLE_ADMIN);
        validate_market(&env, &config);
        let now = env.ledger().timestamp();
        checkpoint_global(&env, now);
        if let Some(mut market) = storage::market(&env, &market_symbol) {
            checkpoint_market(&env, &mut market, now);
            let markets = storage::active_markets(&env);
            let mut hard_sum = 0u64;
            let mut i = 0u32;
            while i < markets.len() {
                let symbol = markets.get(i).unwrap();
                let factor = if symbol == market_symbol {
                    config.hard_cap_pnl_factor_bps
                } else {
                    storage::market(&env, &symbol)
                        .unwrap()
                        .config
                        .hard_cap_pnl_factor_bps
                };
                hard_sum += factor as u64 * 2;
                i += 1;
            }
            if hard_sum > storage::global_config(&env).hard_cap_factor_limit_bps as u64 {
                panic_with_error!(&env, PositionManagerError::InvalidConfig);
            }
            market.config = config;
            recompute_market_flow(&env, &mut market);
            storage::save_market(&env, &market_symbol, &market);
        } else {
            let mut markets = storage::active_markets(&env);
            if markets.len() >= storage::global_config(&env).max_active_markets {
                panic_with_error!(&env, PositionManagerError::MarketLimitExceeded);
            }
            let hard_sum = {
                let mut sum = config.hard_cap_pnl_factor_bps as u64 * 2;
                let mut i = 0u32;
                while i < markets.len() {
                    let s = markets.get(i).unwrap();
                    let m = storage::market(&env, &s).unwrap();
                    sum += m.config.hard_cap_pnl_factor_bps as u64 * 2;
                    i += 1;
                }
                sum
            };
            if hard_sum > storage::global_config(&env).hard_cap_factor_limit_bps as u64 {
                panic_with_error!(&env, PositionManagerError::InvalidConfig);
            }
            storage::save_market(&env, &market_symbol, &empty_market(config, now));
            markets.push_back(market_symbol.clone());
            storage::set(&env, &storage::Key::ActiveMarkets, &markets);
        }
        refresh_rate(&env, physical_cash(&env));
    }

    fn disable_market(env: Env, caller: Address, market: Symbol) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::MarketDisabled(market), &true);
    }

    fn enable_market(env: Env, caller: Address, market: Symbol) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::MarketDisabled(market), &false);
    }

    fn is_market_disabled(env: Env, market: Symbol) -> bool {
        storage::get(&env, &storage::Key::MarketDisabled(market)).unwrap_or(false)
    }

    fn prepare_lp_snapshot(
        env: Env,
        caller: Address,
        round: OracleRound,
        physical: i128,
    ) -> AccountingSnapshot {
        require_vault(&env, &caller);
        checkpoint_global(&env, env.ledger().timestamp());
        let result = build_snapshot(&env, &round, physical, true);
        refresh_rate(&env, physical);
        result
    }

    fn refresh_borrow_rate(env: Env, caller: Address, physical: i128) {
        require_vault(&env, &caller);
        checkpoint_global(&env, env.ledger().timestamp());
        refresh_rate(&env, physical);
    }

    fn can_create_lp_request(env: Env, caller: Address, physical: i128) -> bool {
        require_vault(&env, &caller);
        checkpoint_global(&env, env.ledger().timestamp());
        let claims = claim_total(&env);
        refresh_rate(&env, physical);
        claims <= physical && storage::get_u32(&env, &storage::Key::LpBlockedSideCount) == 0
    }

    fn accounting_snapshot(env: Env, round: OracleRound, physical: i128) -> AccountingSnapshot {
        build_snapshot(&env, &round, physical, false)
    }

    fn get_position(env: Env, position_id: u64) -> Position {
        storage::position(&env, position_id)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound))
    }

    fn get_market(env: Env, market: Symbol) -> MarketInfo {
        storage::market(&env, &market)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::MarketNotConfigured))
    }

    fn active_markets(env: Env) -> Vec<Symbol> {
        storage::active_markets(&env)
    }

    fn global_config(env: Env) -> GlobalConfig {
        storage::global_config(&env)
    }

    fn pending_receiver_funding_total(env: Env) -> i128 {
        storage::get_i128(&env, &storage::Key::PendingReceiverFundingTotal)
    }

    fn protocol_claimable_total(env: Env) -> i128 {
        storage::get_i128(&env, &storage::Key::ProtocolClaimableTotal)
    }

    fn risk_keeper_reserve_total(env: Env) -> i128 {
        storage::get_i128(&env, &storage::Key::RiskKeeperReserveTotal)
    }

    fn non_lp_claims(env: Env) -> i128 {
        claim_total(&env)
    }

    fn claim_protocol(env: Env, caller: Address, recipient: Address, amount: i128) {
        require_role(&env, &caller, ROLE_ADMIN);
        checkpoint_global(&env, env.ledger().timestamp());
        let claim = storage::get_i128(&env, &storage::Key::ProtocolClaimableTotal);
        if amount <= 0 || amount > claim {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        storage::set(
            &env,
            &storage::Key::ProtocolClaimableTotal,
            &math::sub(&env, claim, amount),
        );
        VaultClient::new(&env, &storage::vault(&env)).transfer_claim(
            &env.current_contract_address(),
            &recipient,
            &amount,
            &claim_total(&env),
        );
        refresh_rate(&env, physical_cash(&env));
    }

    fn recapitalize(env: Env, contributor: Address, amount: i128) {
        contributor.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        VaultClient::new(&env, &storage::vault(&env)).receive_collateral(
            &env.current_contract_address(),
            &contributor,
            &amount,
        );
        checkpoint_global(&env, env.ledger().timestamp());
        refresh_rate(&env, physical_cash(&env));
    }

    fn pause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::Paused, &true);
    }

    fn unpause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::Paused, &false);
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
    }

    fn bump_position(env: Env, position_id: u64) {
        let position = storage::position(&env, position_id).unwrap_or_else(|| {
            panic_with_error!(&env, PositionManagerError::PositionNotFound);
        });
        storage::save_position(&env, &position);
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl PositionManagerContract {
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, migration_data: MigrationData, operator: Address) {
        require_role(&env, &operator, ROLE_UPGRADER);
        ensure_can_complete_migration(&env);
        let _ = migration_data;
        complete_migration(&env);
    }
}

impl TimelockedUpgradeable for PositionManagerContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_UPGRADER);
    }
    fn _require_executor(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_UPGRADER);
    }
    fn _require_canceller(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_PAUSER);
    }
    fn _timelock_seconds(env: &Env) -> u64 {
        ConfigManagerClient::new(env, &storage::config_manager(env)).get_upgrade_timelock()
    }
    fn _panic_with_upgrade_error(env: &Env, _: UpgradeFailure) -> ! {
        panic_with_error!(env, PositionManagerError::InvalidCaller)
    }
}
