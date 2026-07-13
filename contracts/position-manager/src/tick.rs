//! Market checkpoint: refresh borrow and dominant-side skew carrying indices,
//! bind a current oracle mark, and evaluate position slices against that state.

use interfaces::{ConfigManagerClient, MarketInfo, OracleRouterClient};
use shared::CarryingFeeConfig;
use soroban_sdk::{Env, Symbol};

use crate::events;
use crate::math;
use crate::storage;
use crate::types::Position;
use crate::vault_view::VaultView;

pub struct MarketTick {
    pub market: MarketInfo,
    pub mark_price: i128,
}

pub struct PositionEvaluation {
    pub pnl: i128,
    pub borrow_fee: i128,
    pub skew_fee: i128,
    pub effective_health: i128,
}

impl MarketTick {
    /// Accrue the elapsed interval against the market state that existed during
    /// that interval. OI mutations happen only after this returns.
    pub fn refresh(env: &Env, symbol: &Symbol, view: &VaultView) -> Self {
        let mut market = storage::get_market(env, symbol);
        let now = env.ledger().timestamp();

        let is_paused = storage::get_paused(env);
        let last_unpause = storage::get_last_unpause_time(env);
        let last_pause = storage::get_last_pause_time(env);
        let effective_start = market.last_index_update.max(last_unpause);
        let effective_now = if is_paused {
            if last_pause > 0 {
                now.min(last_pause)
            } else {
                effective_start
            }
        } else {
            now
        };
        let live_delta = effective_now.saturating_sub(effective_start);
        let pre_pause_delta = if !is_paused && last_pause > 0 && last_unpause >= last_pause {
            last_pause.saturating_sub(market.last_index_update)
        } else {
            0
        };
        let time_delta = live_delta + pre_pause_delta;

        if time_delta > 0 {
            let config = load_carrying_fee_config(env);
            let utilization_bps = view.utilization_bps();
            let borrow_rate = math::calc_borrow_rate(
                utilization_bps,
                config.base_borrow_rate_bps,
                config.slope1_bps,
                config.slope2_bps,
                config.optimal_utilization_bps,
            );
            market.acc_borrow_index =
                math::accumulate_fee_index(env, market.acc_borrow_index, borrow_rate, time_delta);

            let skew_rate_bps = math::calc_skew_rate(
                env,
                market.long_open_interest,
                market.short_open_interest,
                utilization_bps,
                config.max_skew_rate_bps,
            );
            if market.long_open_interest > market.short_open_interest {
                market.acc_long_skew_index = math::accumulate_fee_index(
                    env,
                    market.acc_long_skew_index,
                    skew_rate_bps,
                    time_delta,
                );
            } else if market.short_open_interest > market.long_open_interest {
                market.acc_short_skew_index = math::accumulate_fee_index(
                    env,
                    market.acc_short_skew_index,
                    skew_rate_bps,
                    time_delta,
                );
            }

            market.last_index_update = effective_now;
            storage::set_market(env, symbol, &market);
            events::UpdateIndices {
                symbol: symbol.clone(),
                acc_borrow_index: market.acc_borrow_index,
                acc_long_skew_index: market.acc_long_skew_index,
                acc_short_skew_index: market.acc_short_skew_index,
                skew_rate_bps,
                timestamp: effective_now,
            }
            .publish(env);
        }

        let oracle_addr = storage::get_oracle_router(env);
        let oracle = OracleRouterClient::new(env, &oracle_addr);
        let mark_price = oracle.get_price(symbol);

        Self { market, mark_price }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn evaluate(
        &self,
        env: &Env,
        pos: &Position,
        size: i128,
        collateral: i128,
        base_exposure: i128,
        borrow_fee_debt: i128,
        skew_fee_debt: i128,
    ) -> PositionEvaluation {
        let pnl = math::calc_unrealized_pnl(env, size, base_exposure, self.mark_price, pos.is_long);
        let borrow_fee =
            math::calc_fee_from_debt(env, size, self.market.acc_borrow_index, borrow_fee_debt);
        let skew_index = if pos.is_long {
            self.market.acc_long_skew_index
        } else {
            self.market.acc_short_skew_index
        };
        let skew_fee = math::calc_fee_from_debt(env, size, skew_index, skew_fee_debt);
        let effective_health = math::calc_health(collateral, pnl, borrow_fee, skew_fee);

        PositionEvaluation {
            pnl,
            borrow_fee,
            skew_fee,
            effective_health,
        }
    }

    pub fn is_tp_triggered(&self, take_profit: i128, is_long: bool) -> bool {
        math::is_tp_triggered(take_profit, self.mark_price, is_long)
    }

    pub fn is_sl_triggered(&self, stop_loss: i128, is_long: bool) -> bool {
        math::is_sl_triggered(stop_loss, self.mark_price, is_long)
    }
}

fn load_carrying_fee_config(env: &Env) -> CarryingFeeConfig {
    let config_mgr = storage::get_config_manager(env);
    ConfigManagerClient::new(env, &config_mgr).get_carrying_fee_config()
}
