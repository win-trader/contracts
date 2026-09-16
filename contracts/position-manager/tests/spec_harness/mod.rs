//! The deployed protocol, shared by the spec-derived suites.
//!
//! Built from the `shared` contract interfaces and
//! `docs/design/trading-fees-and-settlement-specification.md`. It owns the
//! wiring of §7.18, the clock-and-observation verbs §8.6 needs kept apart,
//! and the two §9.1 conservation assertions every suite closes with.

#![allow(dead_code)]

use shared::constants::{PRICE_PRECISION, ROLE_PAUSER, ROLE_UNPAUSER};
use shared::{
    defaults, ActionOutcome, LpConfig, OpenPayload, PositionManagerClient, RequestRouterClient,
    VaultClient,
};
use soroban_sdk::{
    testutils::Address as AddressGen,
    testutils::Ledger as _,
    Address, Env, String, Symbol,
};

// ---------------------------------------------------------------------------
// Units. §2.1: prices, USD notionals, base exposure, and cash all share
// `PRICE_PRECISION`, so one dollar and one whole unit are both 10_000_000.
// ---------------------------------------------------------------------------

pub const DOLLAR: i128 = PRICE_PRECISION;

/// `whole` dollars at `PRICE_PRECISION`.
pub fn usd(whole: i128) -> i128 {
    whole * DOLLAR
}

/// §10.4 — every keeper reward starts at `$0.25`.
pub const REWARD: i128 = defaults::KEEPER_REWARD;

/// §11's worked examples price BTC around `$50,000`.
pub const FILL: i128 = usd_const(50_000);
pub const COMMIT_PRICE: i128 = usd_const(49_900);
pub const ACCEPTABLE_MAX: i128 = usd_const(50_100);
pub const SIZE: i128 = usd_const(100_000);
pub const SUBMITTED: i128 = usd_const(5_100);

pub const fn usd_const(whole: i128) -> i128 {
    whole * PRICE_PRECISION
}

/// Enough LP capital that §9.8's capacity condition is never the thing under
/// test: `$10,000` of risk units needs about `$11,765` of cash LP equity at
/// the default `risk_capacity_limit_bps` of 8,500.
pub const LP_SEED: i128 = usd_const(1_000_000);

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// The deployed protocol: config manager, collateral token, price feed,
/// position manager, vault, request router, wired as §7.18 describes.
pub struct Protocol {
    pub env: Env,
    pub pm: Address,
    pub vault: Address,
    pub router: Address,
    pub token: Address,
    pub feed: Address,
    pub config_manager: Address,
    pub market: Symbol,
    pub admin: Address,
    pub trader: Address,
    pub keeper: Address,
    /// Positions are numbered in creation order (§8.13), so the harness can
    /// hand back the id of the one it just opened.
    next_position: std::cell::Cell<u64>,
}

impl Protocol {
    pub fn new() -> Self {
        Self::with_lp_seed(LP_SEED)
    }

    /// A vault seeded with `seed` of LP capital instead of the comfortable
    /// default. §9.8's capacity condition and §3.9's backstop are both stated
    /// against cash LP equity, so neither is reachable while the vault is far
    /// larger than anything the test can do to it.
    pub fn with_lp_seed(seed: i128) -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(1_000_000);

        let admin = Address::generate(&env);
        let trader = Address::generate(&env);
        let keeper = Address::generate(&env);

        let cm = env.register(config_manager::ConfigManagerContract, (admin.clone(),));
        let cm_client = config_manager::ConfigManagerClient::new(&env, &cm);
        // §12.3 splits pause from unpause across two authorities. These tests
        // are about what a pause does, not about who may call it, so one
        // address holds both.
        for role in [ROLE_PAUSER, ROLE_UNPAUSER] {
            cm_client.grant_role(&admin, &Symbol::new(&env, role), &admin);
        }

        // §12.1 — the collateral token must report `PRICE_DECIMALS`.
        let token = env.register(mock_token::MockToken, ());
        mock_token::MockTokenClient::new(&env, &token).initialize(
            &admin,
            &7,
            &String::from_str(&env, "USD Coin"),
            &String::from_str(&env, "USDC"),
        );

        let feed = env.register(mock_oracle::MockOracle, ());
        let pm = env.register(
            position_manager::PositionManagerContract,
            (cm.clone(), feed.clone(), defaults::global_config()),
        );
        let vault = env.register(
            vault::VaultContract,
            (
                token.clone(),
                cm.clone(),
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
            (token.clone(), vault.clone(), cm.clone()),
        );

        let market = Symbol::new(&env, "BTC");
        let p = Protocol {
            env,
            pm,
            vault,
            router,
            token,
            feed,
            config_manager: cm.clone(),
            market,
            admin,
            trader,
            keeper,
            next_position: std::cell::Cell::new(1),
        };
        p.pm().set_vault(&p.admin, &p.vault);
        p.vault_client().set_request_router(&p.admin, &p.router);

        // §7.18 — registration applies at once; a later change waits out the
        // timelock.
        p.publish(FILL);
        p.pm()
            .propose_market_config(&p.admin, &p.market, &defaults::market_config());

        p.mint(&p.admin, seed);
        p.mint(&p.trader, usd(50_000));
        // §15.2 — cash into the vault without minting shares. The LP seed
        // must not be an LP *request*: these tests need equity present before
        // the FIFO queue is exercised.
        p.pm().recapitalize(&p.admin, &seed);
        p
    }

    pub fn pm(&self) -> PositionManagerClient<'_> {
        PositionManagerClient::new(&self.env, &self.pm)
    }

    pub fn vault_client(&self) -> VaultClient<'_> {
        VaultClient::new(&self.env, &self.vault)
    }

    pub fn router_client(&self) -> RequestRouterClient<'_> {
        RequestRouterClient::new(&self.env, &self.router)
    }

    pub fn cash(&self, who: &Address) -> i128 {
        soroban_sdk::token::Client::new(&self.env, &self.token).balance(who)
    }

    /// LP shares are an ordinary fungible token issued by the vault.
    pub fn shares(&self, who: &Address) -> i128 {
        soroban_sdk::token::Client::new(&self.env, &self.vault).balance(who)
    }

    /// §4.1 — the token's view of the vault is the only authoritative cash
    /// balance, which is why every accounting read takes it as an argument.
    pub fn physical(&self) -> i128 {
        self.cash(&self.vault)
    }

    pub fn mint(&self, who: &Address, amount: i128) {
        mock_token::MockTokenClient::new(&self.env, &self.token).admin_mint(
            &self.admin,
            who,
            &amount,
        );
    }

    pub fn snapshot(&self) -> shared::AccountingSnapshot {
        self.pm().accounting_snapshot(&self.physical())
    }

    pub fn now(&self) -> u64 {
        self.env.ledger().timestamp()
    }

    /// Publish an observation stamped with the current ledger time.
    pub fn publish(&self, price: i128) {
        mock_oracle::MockOracleClient::new(&self.env, &self.feed).set_price(&self.market, &price);
    }

    /// Advance the clock and publish a **new** observation. §8.6's cursor is
    /// the feed's stamp, not the transaction's, so a test that only moves the
    /// clock has exercised the delay gate and nothing else.
    pub fn observe(&self, seconds: u64, price: i128) {
        self.wait(seconds);
        self.publish(price);
    }

    /// Advance the clock with no new observation.
    pub fn wait(&self, seconds: u64) {
        self.env.ledger().set_timestamp(self.now() + seconds);
    }

    /// The §11.1 order: a `$100,000` long against `$5,100` of collateral.
    pub fn open_payload(&self, lifetime: u64) -> OpenPayload {
        OpenPayload {
            is_long: true,
            size: SIZE,
            submitted_collateral: SUBMITTED,
            acceptable_price: ACCEPTABLE_MAX,
            expires_at: self.now() + lifetime,
            take_profit: 0,
            stop_loss: 0,
        }
    }

    /// Commit the §11.1 order and settle it at `FILL`. Returns the position id.
    pub fn open_position(&self) -> u64 {
        let trader = self.trader.clone();
        self.open(&trader, true, SIZE, SUBMITTED)
    }

    /// Commit and settle an entry of any shape, returning the new position's
    /// id. The price bound is left disabled — these are book-building opens,
    /// and §8.7's predicate is covered where it is the subject.
    pub fn open(&self, owner: &Address, is_long: bool, size: i128, collateral: i128) -> u64 {
        self.publish(COMMIT_PRICE);
        let mut order = self.open_payload(300);
        order.is_long = is_long;
        order.size = size;
        order.submitted_collateral = collateral;
        order.acceptable_price = 0;
        let action = self.pm().create_market_open(owner, &self.market, &order);
        self.observe(6, FILL);
        assert_eq!(
            self.pm().settle_market_open(&self.keeper, &action),
            ActionOutcome::Executed,
            "the entry the harness seeded was expected to open"
        );
        self.claim_position_id()
    }

    /// Take the id the position just opened was given, and advance the
    /// counter. Positions are numbered in creation order (§8.13), so a test
    /// that opens without going through `open` calls this itself — right
    /// after the settlement it knows succeeded, and never otherwise.
    pub fn claim_position_id(&self) -> u64 {
        let id = self.next_position.get();
        self.next_position.set(id + 1);
        id
    }

    /// Advance the clock in `step`-second slices, publishing and
    /// checkpointing at each one. §4.5's carried divisions are supposed to
    /// make this converge with a single long checkpoint.
    pub fn accrue(&self, total: u64, step: u64, price: i128) {
        let mut elapsed = 0;
        while elapsed < total {
            let slice = step.min(total - elapsed);
            self.observe(slice, price);
            self.pm().update_indices(&self.keeper, &self.market);
            elapsed += slice;
        }
    }

    /// §7.17 — `prepare_lp_snapshot` is the vault's alone, and it is what
    /// "evaluates and persists every side's risk state". Resolving an LP
    /// request is the way to reach it through the public interface. §14
    /// refuses a new request once a side is restricted, so this works to
    /// enter a restricted state and not to leave one.
    pub fn latch_risk_state(&self, price: i128) {
        let lp = Address::generate(&self.env);
        self.mint(&lp, usd(1_000));
        self.router_client().request_deposit(&lp, &usd(1_000));
        self.observe(defaults::LP_REQUEST_DELAY_LOCAL, price);
        let executor = Address::generate(&self.env);
        self.router_client().resolve_next(&executor);
    }

    /// §9.1 — physical cash is exactly divided between LP equity and the
    /// non-LP labels, and a deficit is reported rather than concealed.
    pub fn assert_conserved(&self, at: &str) {
        let s = self.snapshot();
        assert_eq!(
            s.physical_cash,
            self.physical(),
            "{at}: §4.1 — the snapshot's cash is the token balance"
        );
        if s.cash_shortfall == 0 {
            assert_eq!(
                s.physical_cash,
                s.cash_lp_equity + s.non_lp_claims,
                "{at}: §9.1 — physical_cash = cash_lp_equity + non_lp_claims"
            );
        } else {
            assert_eq!(s.cash_lp_equity, 0, "{at}: §9.1 — a shortfall zeroes equity");
            assert_eq!(
                s.cash_shortfall,
                s.non_lp_claims - s.physical_cash,
                "{at}: §9.1 — the deficit is reported, not hidden"
            );
        }
        assert!(
            s.non_lp_claims >= 0 && s.cash_lp_equity >= 0,
            "{at}: §9.1 — no label may go negative to absorb a deficit"
        );
    }

    /// §2.5's decomposition of `non_lp_claims` into its six labels. The
    /// interface exposes no
    /// aggregate for position collateral or action escrow, so the caller
    /// names the live positions and pending actions.
    pub fn claims_from_parts(&self, positions: &[u64], actions: &[u64]) -> i128 {
        let c = self.pm();
        let mut total = c.pending_receiver_funding_total()
            + c.protocol_claimable_total()
            + c.referral_claimable_total()
            + c.unclaimed_payout_total();
        for id in positions {
            total += c.get_position(id).stored_collateral;
        }
        for id in actions {
            total += c.get_pending_action(id).escrowed_collateral;
        }
        total
    }
}

