pub mod invariants;

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger, LedgerInfo},
    vec, Address, Env, Symbol,
};

use config_manager::{ConfigManagerClient, ConfigManagerContract};
use mock_oracle::{MockOracle, MockOracleClient};
use mock_token::{MockToken, MockTokenClient};
use oracle_router::{OracleConfig, OracleRouterClient, OracleRouterContract};
use position_manager::{PositionManagerClient, PositionManagerContract};
use shared::constants::{ROLE_KEEPER, ROLE_PAUSER};
use vault::{VaultContract, VaultContractClient};

/// BTC price: $50,000 scaled by 1e7
pub const BTC_PRICE: i128 = 500_000_000_000; // 50_000 * 1e7

/// 1 USDC = 1_000_000 (6 decimals)
pub const USDC_UNIT: i128 = 1_000_000;

/// Default vault deposit: 1,000,000 USDC
pub const VAULT_DEPOSIT: i128 = 1_000_000 * USDC_UNIT;

/// Trader starting balance: 100,000 USDC
pub const TRADER_BALANCE: i128 = 100_000 * USDC_UNIT;

/// Ledger timestamp
pub const TEST_TIMESTAMP: u64 = 1_700_000_000;

/// Two mock oracle sources driven in lockstep. The OracleRouter enforces a
/// quorum floor of two sources, so the fixture registers a pair and moves
/// both together: `set_price` fans out to both, keeping the median exact and
/// the deviation zero.
pub struct DualMock<'a> {
    pub a: MockOracleClient<'a>,
    pub b: MockOracleClient<'a>,
}

impl DualMock<'_> {
    pub fn set_price(&self, symbol: &Symbol, price: &i128) {
        self.a.set_price(symbol, price);
        self.b.set_price(symbol, price);
    }
}

/// A fully-deployed protocol fixture for integration tests.
///
/// Call `Fixture::deploy(&env)` at the start of each test to get a fresh,
/// pre-configured set of contracts wired together and ready to use.
pub struct Fixture<'a> {
    pub env: &'a Env,
    pub admin: Address,
    pub keeper: Address,
    pub trader: Address,
    pub config_manager: ConfigManagerClient<'a>,
    pub vault: VaultContractClient<'a>,
    pub position_manager: PositionManagerClient<'a>,
    pub oracle_router: OracleRouterClient<'a>,
    pub usdc: MockTokenClient<'a>,
    pub mock_oracle: DualMock<'a>,
    pub pm_addr: Address,
    pub vault_addr: Address,
}

impl<'a> Fixture<'a> {
    /// Deploy all protocol contracts, configure roles, and wire them together.
    pub fn deploy(env: &'a Env) -> Self {
        env.mock_all_auths();

        env.ledger().set(LedgerInfo {
            timestamp: TEST_TIMESTAMP,
            protocol_version: 23,
            sequence_number: 100,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 100,
            max_entry_ttl: 10_000_000,
        });

        let admin = Address::generate(env);
        let keeper = Address::generate(env);
        let trader = Address::generate(env);
        let lp = Address::generate(env);

        // 1. ConfigManager — grant roles
        let config_id = env.register(ConfigManagerContract, (admin.clone(),));
        let config_manager = ConfigManagerClient::new(env, &config_id);

        let pauser_role = Symbol::new(env, ROLE_PAUSER);
        let keeper_role = Symbol::new(env, ROLE_KEEPER);
        config_manager.grant_role(&admin, &pauser_role, &admin);
        config_manager.grant_role(&admin, &keeper_role, &admin);
        config_manager.grant_role(&admin, &keeper_role, &keeper);

        // Set fee splits: 90% LP, 10% dev, 0% stakers.
        config_manager.update_fee_splits(
            &admin,
            &config_manager::FeeSplits {
                lp_bps: 9000,
                dev_bps: 1000,
                staker_bps: 0,
            },
        );

        // Set protocol limits
        config_manager.update_protocol_limits(
            &admin,
            &config_manager::ProtocolLimits {
                min_collateral: 1_000_000,
                cooldown_duration: 60,
                min_position_lifetime: 60,
                max_utilization_ratio: 8_500,
                funding_cut_bps: 500,
                adl_pnl_bps: 9_000,
                adl_utilization_bps: 9_500,
                liquidation_threshold_bps: 200,
            },
        );

        config_manager.update_borrow_rate_config(
            &admin,
            &config_manager::BorrowRateConfig {
                base_borrow_rate_bps: 100,
                slope1_bps: 500,
                slope2_bps: 5_000,
                optimal_utilization_bps: 8_000,
                base_funding_rate_bps: 100,
            },
        );

        // 2. MockToken (USDC) + MockOracle
        let usdc_id = env.register(MockToken, ());
        let usdc = MockTokenClient::new(env, &usdc_id);
        usdc.initialize(
            &admin,
            &6u32,
            &soroban_sdk::String::from_str(env, "USD Coin"),
            &soroban_sdk::String::from_str(env, "USDC"),
        );

        let oracle_id = env.register(MockOracle, ());
        let mock_oracle_a = MockOracleClient::new(env, &oracle_id);
        mock_oracle_a.initialize();
        mock_oracle_a.set_price(&symbol_short!("BTC"), &BTC_PRICE);

        let oracle_id_b = env.register(MockOracle, ());
        let mock_oracle_b = MockOracleClient::new(env, &oracle_id_b);
        mock_oracle_b.initialize();
        mock_oracle_b.set_price(&symbol_short!("BTC"), &BTC_PRICE);

        let mock_oracle = DualMock {
            a: mock_oracle_a,
            b: mock_oracle_b,
        };

        // 3. OracleRouter — link to ConfigManager + both MockOracle sources
        let oracle_router_id = env.register(OracleRouterContract, (config_id.clone(),));
        let oracle_router = OracleRouterClient::new(env, &oracle_router_id);
        oracle_router.set_oracle_config(
            &admin,
            &OracleConfig {
                max_deviation_bps: 500,
                staleness_threshold: 3600,
                cache_duration: 10,
                min_required_sources: 2,
            },
        );
        oracle_router.set_oracle_sources(
            &admin,
            &symbol_short!("BTC"),
            &vec![env, oracle_id.clone(), oracle_id_b.clone()]);

        // 4. PositionManager — bind ConfigManager + OracleRouter at construction.
        //    Registered before the Vault so the Vault can bind this PM address
        //    atomically in its own constructor (the Vault↔PM cycle is closed by
        //    the one-shot `set_vault` below).
        let pm_id = env.register(
            PositionManagerContract,
            (config_id.clone(), oracle_router_id.clone()),
        );
        let position_manager = PositionManagerClient::new(env, &pm_id);

        // 5. Vault — binds asset + ConfigManager + the trusted PositionManager
        //    atomically at construction.
        let vault_id = env.register(
            VaultContract,
            (usdc_id.clone(), config_id.clone(), pm_id.clone()),
        );
        let vault = VaultContractClient::new(env, &vault_id);

        // 6. Wire the Vault into PositionManager (one-shot, ADMIN-gated).
        position_manager.set_vault(&admin, &vault_id);

        // Set per-market max leverage
        position_manager.set_max_leverage(&admin, &symbol_short!("BTC"), &100_i128);

        // Fund accounts
        usdc.mint(&trader, &TRADER_BALANCE);
        usdc.mint(&lp, &VAULT_DEPOSIT);
        vault.deposit(&VAULT_DEPOSIT, &lp, &lp, &lp);

        Fixture {
            env,
            admin,
            keeper,
            trader,
            config_manager,
            vault,
            position_manager,
            oracle_router,
            usdc,
            mock_oracle,
            pm_addr: pm_id,
            vault_addr: vault_id,
        }
    }

    /// Create a funded trader address with the given USDC balance.
    pub fn create_funded_trader(&self, amount: i128) -> Address {
        let trader = Address::generate(self.env);
        self.usdc.mint(&trader, &amount);
        trader
    }

    /// Shorthand: open a long BTC position. Asserts protocol invariants on
    /// success — every state-changing wrapper does, see `invariants::assert_protocol_invariants`.
    pub fn open_long(&self, trader: &Address, size: i128, collateral: i128) {
        self.position_manager.increase_position(
            trader,
            &symbol_short!("BTC"),
            &size,
            &collateral,
            &true,
            &0,
            &0,
            &0i128,
        );
        invariants::assert_protocol_invariants(self.env, self, "open_long");
    }

    /// Shorthand: open a short BTC position.
    pub fn open_short(&self, trader: &Address, size: i128, collateral: i128) {
        self.position_manager.increase_position(
            trader,
            &symbol_short!("BTC"),
            &size,
            &collateral,
            &false,
            &0,
            &0,
            &0i128,
        );
        invariants::assert_protocol_invariants(self.env, self, "open_short");
    }

    // -----------------------------------------------------------------------
    // PositionManager wrappers — every state-changing op runs the invariant
    // check immediately after. Direct calls through `f.position_manager.X(...)`
    // still work but skip the check; prefer these wrappers.
    // -----------------------------------------------------------------------

    /// Open a position with full control over `is_long`, TP/SL, and slippage.
    /// Arguments mirror the soroban client signature (all by reference) so
    /// callers can `s/f.position_manager.increase_position/f.increase_position/`
    /// without further changes.
    #[allow(clippy::too_many_arguments)]
    pub fn increase_position(
        &self,
        trader: &Address,
        symbol: &Symbol,
        size: &i128,
        collateral: &i128,
        is_long: &bool,
        take_profit: &i128,
        stop_loss: &i128,
        acceptable_price: &i128,
    ) {
        self.position_manager.increase_position(
            trader,
            symbol,
            size,
            collateral,
            is_long,
            take_profit,
            stop_loss,
            acceptable_price,
        );
        invariants::assert_protocol_invariants(self.env, self, "increase_position");
    }

    /// Reduce or fully close a position.
    pub fn decrease_position(
        &self,
        trader: &Address,
        symbol: &Symbol,
        size_delta: &i128,
        acceptable_price: &i128,
    ) {
        self.position_manager.decrease_position(trader, symbol, size_delta, acceptable_price);
        invariants::assert_protocol_invariants(self.env, self, "decrease_position");
    }

    /// Force-close an undercollateralized position (KEEPER).
    pub fn liquidate(&self, caller: &Address, trader: &Address, symbol: &Symbol) {
        self.position_manager.liquidate_position(caller, trader, symbol);
        invariants::assert_protocol_invariants(self.env, self, "liquidate");
    }

    /// Execute a TP/SL order (KEEPER).
    pub fn execute_order(&self, caller: &Address, trader: &Address, symbol: &Symbol) {
        self.position_manager.execute_order(caller, trader, symbol);
        invariants::assert_protocol_invariants(self.env, self, "execute_order");
    }

    /// Auto-deleverage (KEEPER).
    pub fn deleverage_position(&self, caller: &Address, trader: &Address, symbol: &Symbol) {
        self.position_manager.deleverage_position(caller, trader, symbol);
        invariants::assert_protocol_invariants(self.env, self, "deleverage_position");
    }

    /// Sync borrow/funding accumulators (KEEPER).
    pub fn update_indices(&self, caller: &Address, symbol: &Symbol) {
        self.position_manager.update_indices(caller, symbol);
        invariants::assert_protocol_invariants(self.env, self, "update_indices");
    }

    /// Set or clear TP/SL on an existing position.
    pub fn set_tp_sl(
        &self,
        trader: &Address,
        symbol: &Symbol,
        take_profit: &i128,
        stop_loss: &i128,
    ) {
        self.position_manager.set_tp_sl(trader, symbol, take_profit, stop_loss);
        invariants::assert_protocol_invariants(self.env, self, "set_tp_sl");
    }

    /// Set per-market max leverage (ADMIN).
    pub fn set_max_leverage(&self, caller: &Address, symbol: &Symbol, max_leverage: &i128) {
        self.position_manager.set_max_leverage(caller, symbol, max_leverage);
        invariants::assert_protocol_invariants(self.env, self, "set_max_leverage");
    }

    /// Pause the position manager (PAUSER).
    pub fn pause_pm(&self, caller: &Address) {
        self.position_manager.pause(caller);
        invariants::assert_protocol_invariants(self.env, self, "pause_pm");
    }

    /// Unpause the position manager (PAUSER).
    pub fn unpause_pm(&self, caller: &Address) {
        self.position_manager.unpause(caller);
        invariants::assert_protocol_invariants(self.env, self, "unpause_pm");
    }

    /// Disable a market for opens (PAUSER).
    pub fn disable_market(&self, caller: &Address, symbol: &Symbol) {
        self.position_manager.disable_market(caller, symbol);
        invariants::assert_protocol_invariants(self.env, self, "disable_market");
    }

    /// Re-enable a previously-disabled market (PAUSER).
    pub fn enable_market(&self, caller: &Address, symbol: &Symbol) {
        self.position_manager.enable_market(caller, symbol);
        invariants::assert_protocol_invariants(self.env, self, "enable_market");
    }

    // -----------------------------------------------------------------------
    // Vault wrappers
    // -----------------------------------------------------------------------

    /// Deposit USDC into the vault and receive LP shares.
    pub fn deposit(
        &self,
        assets: &i128,
        receiver: &Address,
        from: &Address,
        operator: &Address,
    ) -> i128 {
        let shares = self.vault.deposit(assets, receiver, from, operator);
        invariants::assert_protocol_invariants(self.env, self, "deposit");
        shares
    }

    /// Withdraw USDC from the vault by burning shares.
    pub fn withdraw(
        &self,
        assets: &i128,
        receiver: &Address,
        owner: &Address,
        operator: &Address,
    ) -> i128 {
        let shares = self.vault.withdraw(assets, receiver, owner, operator);
        invariants::assert_protocol_invariants(self.env, self, "withdraw");
        shares
    }

    /// Mint exact shares and pay the corresponding USDC.
    pub fn mint_vault(
        &self,
        shares: &i128,
        receiver: &Address,
        from: &Address,
        operator: &Address,
    ) -> i128 {
        let assets = self.vault.mint(shares, receiver, from, operator);
        invariants::assert_protocol_invariants(self.env, self, "mint_vault");
        assets
    }

    /// Redeem exact shares and receive USDC.
    pub fn redeem(
        &self,
        shares: &i128,
        receiver: &Address,
        owner: &Address,
        operator: &Address,
    ) -> i128 {
        let assets = self.vault.redeem(shares, receiver, owner, operator);
        invariants::assert_protocol_invariants(self.env, self, "redeem");
        assets
    }

    /// Pause the vault (PAUSER).
    pub fn pause_vault(&self, caller: &Address) {
        self.vault.pause(caller);
        invariants::assert_protocol_invariants(self.env, self, "pause_vault");
    }

    /// Unpause the vault (PAUSER).
    pub fn unpause_vault(&self, caller: &Address) {
        self.vault.unpause(caller);
        invariants::assert_protocol_invariants(self.env, self, "unpause_vault");
    }

    /// Claim all accrued fees to `recipient` (ADMIN).
    pub fn claim_fees(&self, caller: &Address, recipient: &Address) {
        self.vault.claim_fees(caller, recipient);
        invariants::assert_protocol_invariants(self.env, self, "claim_fees");
    }

    /// Set the BTC oracle price. `price_usd` is in whole dollars (e.g. 50_000).
    ///
    /// Oracle price changes do not directly mutate vault/PM state but do
    /// shift unrealized PnL on the next read. The invariants helper reads
    /// PM's `total_unrealized_pnl` lazily, so this is a state-change for
    /// invariant purposes — but we deliberately do NOT assert here, because
    /// `vault.net_global_trader_pnl` is only refreshed by PM calls
    /// (`update_indices`, `increase_position`, `decrease_position`, …). The
    /// drift between `pm.total_unrealized` and `vault.net` is real and
    /// expected between an oracle move and the next PM call — clause 5 of
    /// the invariant would (correctly) fire here. Tests that move the
    /// oracle should follow up with `update_indices` (or any PM op) before
    /// the next assertion.
    pub fn set_btc_price(&self, price_usd: i128) {
        let scaled = price_usd * 10_000_000;
        self.mock_oracle.set_price(&symbol_short!("BTC"), &scaled);
    }

    /// Advance the ledger timestamp. Like `set_btc_price`, this is a
    /// pre-state-change setup step and does not trigger an invariant check.
    pub fn advance_time(&self, new_ts: u64) {
        self.env.ledger().set(LedgerInfo {
            timestamp: new_ts,
            protocol_version: 23,
            sequence_number: 100 + ((new_ts - TEST_TIMESTAMP) as u32),
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 100,
            max_entry_ttl: 10_000_000,
        });
    }
}
