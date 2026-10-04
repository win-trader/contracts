//! The deployed-and-wired protocol, shared by the integration suites.
//!
//! Deliberately the real types rather than a wire-format clean room: these
//! suites are about behaviour, and the ABI is checked by the fact that this
//! compiles against `shared`.

#![allow(dead_code)]

use position_manager::PositionManagerContractClient;
use position_manager::PositionManagerContract;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String, Symbol,
};

use shared::constants::{ROLE_PAUSER, ROLE_UNPAUSER};
use shared::{
    defaults, GlobalConfig, LpConfig, OpenPayload, 
};

pub const PRICE: i128 = 100_000_0000000; // $100_000 at PRICE_PRECISION
const DECIMALS: u32 = 7;

/// The whole protocol, deployed and wired.
pub struct World {
    pub env: Env,
    pub pm: Address,
    pub feed: Address,
    pub token: Address,
    pub market: Symbol,
    pub vault: Address,
    pub router: Address,
    pub admin: Address,
    pub trader: Address,
    pub keeper: Address,
}

impl World {
    pub fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(1_000_000);

        let admin = Address::generate(&env);
        let trader = Address::generate(&env);
        let keeper = Address::generate(&env);

        let config_manager = env.register(config_manager::ConfigManagerContract, (admin.clone(),));
        // One address holds every authority; these tests are about the
        // lifecycle, not about who may call what.
        let cm = config_manager::ConfigManagerClient::new(&env, &config_manager);
        for role in [ROLE_PAUSER, ROLE_UNPAUSER] {
            cm.grant_role(&admin, &Symbol::new(&env, role), &admin);
        }
        let token = env.register(mock_token::MockToken, ());
        mock_token::MockTokenClient::new(&env, &token).initialize(
            &admin,
            &DECIMALS,
            &String::from_str(&env, "USD Coin"),
            &String::from_str(&env, "USDC"),
        );
        let feed = env.register(mock_oracle::MockOracle, ());

        let pm = env.register(
            PositionManagerContract,
            (config_manager.clone(), feed.clone(), global_config()),
        );
        let vault = env.register(
            vault::VaultContract,
            (
                token.clone(),
                config_manager.clone(),
                pm.clone(),
                LpConfig {
                    max_withdraw_utilization_bps: defaults::MAX_WITHDRAW_UTILIZATION_BPS,
                    min_deposit_nav_factor_bps: defaults::MIN_DEPOSIT_NAV_FACTOR_BPS,
                    lp_request_delay_seconds: defaults::LP_REQUEST_DELAY_LOCAL,
                },
            ),
        );

        let router = env.register(
            request_router::RequestRouterContract,
            (token.clone(), vault.clone(), config_manager.clone()),
        );
        let client = PositionManagerContractClient::new(&env, &pm);
        client.set_vault(&admin, &vault);
        shared::VaultClient::new(&env, &vault).set_request_router(&admin, &router);

        let market = Symbol::new(&env, "BTC");
        mock_oracle::MockOracleClient::new(&env, &feed).set_price(&market, &PRICE);
        client.propose_market_config(&admin, &market, &defaults::market_config());

        // Seed the vault with LP capital and the trader with spending money.
        let token_client = mock_token::MockTokenClient::new(&env, &token);
        token_client.admin_mint(&admin, &admin, &10_000_000_000_000);
        token_client.admin_mint(&admin, &trader, &10_000_000_000);
        client.recapitalize(&admin, &1_000_000_000_000);

        World {
            env,
            pm,
            feed,
            token,
            market,
            vault,
            router,
            admin,
            trader,
            keeper,
        }
    }

    pub fn client(&self) -> PositionManagerContractClient<'_> {
        PositionManagerContractClient::new(&self.env, &self.pm)
    }

    pub fn balance(&self, who: &Address) -> i128 {
        mock_token::MockTokenClient::new(&self.env, &self.token).balance(who)
    }

    /// §4.1 — the only authoritative cash balance is the token's view of the
    /// vault. Every accounting read takes it as an argument rather than
    /// caching it.
    pub fn physical(&self) -> i128 {
        self.balance(&self.vault)
    }

    pub fn mint(&self, who: &Address, amount: i128) {
        mock_token::MockTokenClient::new(&self.env, &self.token)
            .admin_mint(&self.admin, who, &amount);
    }

    /// Advance the ledger clock and publish a **new observation** at
    /// `price`. Both halves matter: §8.6's cursor is the feed's stamp, not
    /// the transaction's, so a test that only moves the clock is testing the
    /// delay gate and nothing else.
    pub fn observe(&self, seconds: u64, price: i128) {
        let now = self.env.ledger().timestamp() + seconds;
        self.env.ledger().set_timestamp(now);
        mock_oracle::MockOracleClient::new(&self.env, &self.feed).set_price(&self.market, &price);
    }

    /// Advance the clock **without** a new observation.
    pub fn wait(&self, seconds: u64) {
        let now = self.env.ledger().timestamp() + seconds;
        self.env.ledger().set_timestamp(now);
    }

    pub fn request(&self, collateral: i128, acceptable_price: i128, expires_in: u64) -> OpenPayload {
        OpenPayload {
            is_long: true,
            // $1,000 of notional at 5% initial margin needs $50.
            size: 1_000_0000000,
            submitted_collateral: collateral,
            acceptable_price,
            expires_at: self.env.ledger().timestamp() + expires_in,
            take_profit: 0,
            stop_loss: 0,
        }
    }
}

pub fn global_config() -> GlobalConfig {
    defaults::global_config()
}

