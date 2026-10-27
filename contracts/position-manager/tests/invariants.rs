//! Accounting invariants under a seeded random walk (THREAT_MODEL T-18).
//!
//! Every trader, keeper, LP and governance path the protocol exposes is driven
//! in a reproducible pseudo-random order across several seeds. After every
//! step the whole book is reconciled, not just total cash:
//!
//! - §9.1: physical cash = LP equity + non-LP claims (or the shortfall is reported);
//! - §2.5: non-LP claims equal the sum of their parts, position by position
//!   and action by action;
//! - §9.7: every market-side aggregate and the ledger's risk total and
//!   position count equal the sum over the live positions;
//! - the request router holds exactly the escrow of its pending requests plus
//!   the payouts it is holding.
//!
//! A failure prints the seed and step, so it reproduces exactly.

mod spec_harness;

use spec_harness::*;

use shared::constants::ROLE_PROTOCOL;
use shared::{defaults, ActionOutcome, LpRequestKind, LpRequestStatus};
use soroban_sdk::{testutils::Address as _, Address, Symbol};

const SEEDS: [u64; 6] = [
    0x5DEE_CE66_D1CE_B00D,
    0x0123_4567_89AB_CDEF,
    0xDEAD_BEEF_CAFE_F00D,
    0x0F0F_0F0F_F0F0_F0F0,
    0x1357_9BDF_2468_ACE0,
    0x7777_0000_3333_9999,
];
const STEPS: u32 = 80;
const PRICES: [i128; 7] = [
    usd_const(50_000),
    usd_const(47_000),
    usd_const(53_500),
    usd_const(44_000),
    usd_const(58_000),
    usd_const(50_500),
    usd_const(61_000),
];

struct Walk<'a> {
    p: &'a Protocol,
    traders: Vec<Address>,
    lps: Vec<Address>,
    /// Every position id ever opened; liveness is re-read from the contract.
    max_position_id: u64,
    /// Every action id created; pending-ness is re-read from the contract.
    actions: Vec<u64>,
    lp_requests: Vec<u64>,
}

impl Walk<'_> {
    fn live_positions(&self) -> Vec<u64> {
        let c = self.p.pm();
        (1..=self.max_position_id).filter(|id| c.try_get_position(id).is_ok()).collect()
    }

    fn pending_actions(&self) -> Vec<u64> {
        let c = self.p.pm();
        self.actions.iter().copied().filter(|id| c.try_get_pending_action(id).is_ok()).collect()
    }

    fn check(&self, at: &str) {
        let p = self.p;
        let c = p.pm();
        p.assert_conserved(at);

        let live = self.live_positions();
        let pending = self.pending_actions();
        let s = p.snapshot();
        assert_eq!(
            s.non_lp_claims,
            p.claims_from_parts(&live, &pending),
            "{at}: §2.5 — non-LP claims equal the sum of their parts"
        );
        assert!(c.pending_receiver_funding_total() >= 0, "{at}: §9.4");
        assert!(c.protocol_claimable_total() >= 0, "{at}: §9.2");
        assert!(c.unclaimed_payout_total() >= 0, "{at}: §12.1");

        // §9.7 — aggregates are exactly the sum over live positions.
        let market = c.get_market(&p.market);
        let mut total_risk = 0i128;
        for (is_long, side) in [(true, &market.long), (false, &market.short)] {
            let (mut size, mut base, mut collateral, mut risk) = (0i128, 0i128, 0i128, 0i128);
            for id in &live {
                let pos = c.get_position(id);
                if pos.is_long == is_long {
                    size += pos.size;
                    base += pos.base_exposure;
                    collateral += pos.stored_collateral;
                    risk += pos.risk_units;
                }
            }
            let name = if is_long { "long" } else { "short" };
            assert_eq!(side.size_open_interest, size, "{at}: {name} size open interest");
            assert_eq!(side.base_exposure, base, "{at}: {name} base exposure");
            assert_eq!(side.stored_collateral_total, collateral, "{at}: {name} stored collateral");
            assert_eq!(side.risk_units, risk, "{at}: {name} risk units");
            total_risk += risk;
        }
        assert_eq!(s.total_risk_units, total_risk, "{at}: ledger risk units");
        assert_eq!(s.open_position_count, live.len() as u64, "{at}: open position count");

        // The router holds exactly its pending escrow plus held payouts.
        let r = p.router_client();
        let (mut assets, mut shares) = (0i128, 0i128);
        for id in &self.lp_requests {
            let req = r.get_request(id);
            if req.status == LpRequestStatus::Pending {
                match req.kind {
                    LpRequestKind::Deposit => assets += req.amount,
                    LpRequestKind::Withdrawal => shares += req.amount,
                }
            }
        }
        for lp in &self.lps {
            assets += r.lp_payout_claimable(lp);
        }
        assert_eq!(p.cash(&p.router), assets, "{at}: router asset escrow");
        assert_eq!(p.shares(&p.router), shares, "{at}: router share escrow");
    }

    fn open(&mut self, r: u32, price: i128, limit: bool) {
        let p = self.p;
        let c = p.pm();
        let who = self.traders[(r as usize / 3) % self.traders.len()].clone();
        p.observe(1, price);
        let mut order = p.open_payload(if limit { 3_600 } else { 300 });
        order.is_long = r % 2 == 0;
        order.acceptable_price = 0;
        order.size = usd(10_000 + (r % 6) as i128 * 8_000);
        order.submitted_collateral = usd(2_000 + (r % 4) as i128 * 1_000);
        let created = if limit {
            // A trigger a little away from the market, on either side.
            let trigger = if r % 4 < 2 { price - price / 50 } else { price + price / 50 };
            c.try_create_limit_open(&who, &p.market, &order, &trigger)
        } else {
            c.try_create_market_open(&who, &p.market, &order)
        };
        let Ok(Ok(action)) = created else { return };
        self.actions.push(action);
        if !limit {
            p.observe(6, price);
            if c.try_settle_market_open(&p.keeper, &action) == Ok(Ok(ActionOutcome::Executed)) {
                self.max_position_id += 1;
            }
        }
    }

    fn sweep_entries(&mut self, r: u32, price: i128) {
        let p = self.p;
        let c = p.pm();
        p.observe(6, price);
        for action in self.pending_actions() {
            let Ok(Ok(a)) = c.try_get_pending_action(&action) else { continue };
            // Keepers retry mutations a price move left non-terminal.
            match a.kind {
                shared::ActionKind::Decrease => {
                    let _ = c.try_settle_decrease(&p.keeper, &action);
                    continue;
                }
                shared::ActionKind::Close => {
                    let _ = c.try_settle_close(&p.keeper, &action);
                    continue;
                }
                _ => {}
            }
            if r % 5 == 0 {
                let _ = c.try_cancel_limit_open(&action);
                continue;
            }
            match c.try_settle_limit_open(&p.keeper, &action) {
                Ok(Ok(ActionOutcome::Executed)) => self.max_position_id += 1,
                Ok(Ok(ActionOutcome::Expired)) => {
                    let _ = c.try_clean_expired_entry(&p.keeper, &action);
                }
                _ => {}
            }
        }
    }

    fn mutate(&mut self, r: u32, price: i128) {
        let p = self.p;
        let c = p.pm();
        let live = self.live_positions();
        if live.is_empty() {
            return;
        }
        let id = live[r as usize % live.len()];
        p.observe(defaults::MIN_POSITION_LIFETIME, price);
        let size = c.get_position(&id).size;
        let created = match r % 3 {
            0 => c.try_create_decrease(&id, &(size / 3), &0),
            _ => c.try_create_close(&id, &0),
        };
        let Ok(Ok(action)) = created else { return };
        self.actions.push(action);
        p.observe(6, price);
        let _ = if r % 3 == 0 {
            c.try_settle_decrease(&p.keeper, &action)
        } else {
            c.try_settle_close(&p.keeper, &action)
        };
    }

    fn triggers(&mut self, r: u32, price: i128) {
        let p = self.p;
        let c = p.pm();
        let live = self.live_positions();
        if live.is_empty() {
            return;
        }
        let id = live[r as usize % live.len()];
        p.observe(1, price);
        let long = c.get_position(&id).is_long;
        let (tp, sl) = if long {
            (price + price / 40, price - price / 40)
        } else {
            (price - price / 40, price + price / 40)
        };
        let _ = c.try_set_take_profit(&id, &tp, &0);
        let _ = c.try_set_stop_loss(&id, &sl, &0);
        for id in self.live_positions() {
            p.observe(6, price);
            let _ = c.try_execute_take_profit(&p.keeper, &id);
            let _ = c.try_execute_stop_loss(&p.keeper, &id);
        }
    }

    fn forced(&mut self, price: i128) {
        let p = self.p;
        let c = p.pm();
        p.observe(1, price);
        for id in self.live_positions() {
            let _ = c.try_liquidate_position(&p.keeper, &id);
        }
        for id in self.live_positions() {
            let _ = c.try_execute_adl(&p.keeper, &id);
        }
    }

    fn lp(&mut self, r: u32, price: i128) {
        let p = self.p;
        let rc = p.router_client();
        let lp = self.lps[r as usize % self.lps.len()].clone();
        p.observe(1, price);
        let request = if r % 2 == 0 || p.shares(&lp) == 0 {
            rc.try_request_deposit(&lp, &usd(5_000 + (r % 4) as i128 * 5_000))
        } else {
            let shares = p.shares(&lp) / 2;
            rc.try_request_withdrawal(&lp, &shares)
        };
        if let Ok(Ok(id)) = request {
            self.lp_requests.push(id);
        }
        p.observe(defaults::LP_REQUEST_DELAY_LOCAL, price);
        while let Ok(Ok(result)) = rc.try_resolve_next(&p.keeper) {
            if result.status == shared::SettlementStatus::NotReady {
                break;
            }
        }
    }

    fn admin(&mut self, r: u32, price: i128) {
        let p = self.p;
        let c = p.pm();
        p.observe(1, price);
        if r % 2 == 0 {
            // A conservative change installs immediately through the governor.
            let mut config = c.global_config();
            if config.risk_capacity_limit_bps > 5_000 {
                config.risk_capacity_limit_bps -= 100;
                p.gov().propose_global_config(&p.admin, &config);
            }
        } else {
            let claimable = c.protocol_claimable_total();
            if claimable > 1 {
                let recipient = Address::generate(&p.env);
                let _ = c.try_claim_protocol(&p.admin, &recipient, &(claimable / 2));
            }
        }
    }
}

fn walk(seed: u64) {
    let p = Protocol::new();
    config_manager::ConfigManagerClient::new(&p.env, &p.config_manager).grant_role(
        &p.admin,
        &Symbol::new(&p.env, ROLE_PROTOCOL),
        &p.admin,
    );
    let traders: Vec<Address> = (0..4).map(|_| Address::generate(&p.env)).collect();
    let lps: Vec<Address> = (0..2).map(|_| Address::generate(&p.env)).collect();
    for who in traders.iter().chain(lps.iter()) {
        p.mint(who, usd(1_000_000));
    }
    let mut w = Walk { p: &p, traders, lps, max_position_id: 0, actions: Vec::new(), lp_requests: Vec::new() };

    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as u32
    };

    for step in 0..STEPS {
        let r = next();
        let price = PRICES[(r as usize / 11) % PRICES.len()];
        let op = r % 10;
        match op {
            0 | 1 => w.open(r, price, false),
            2 => w.open(r, price, true),
            3 => w.sweep_entries(r, price),
            4 => w.mutate(r, price),
            5 => w.triggers(r, price),
            6 => w.forced(price),
            7 => w.lp(r, price),
            8 => w.admin(r, price),
            _ => {
                p.observe(900, price);
                p.pm().update_indices(&p.keeper, &p.market);
            }
        }
        w.check(&format!("seed {seed:#x} step {step} (op {op})"));
    }

    // Unwind: every surviving position closes, and nothing is stranded.
    for id in w.live_positions() {
        p.observe(defaults::MIN_POSITION_LIFETIME, PRICES[0]);
        let c = p.pm();
        if let Some(pending) = c.get_position(&id).pending_mutation_action_id {
            p.observe(6, PRICES[0]);
            let _ = match c.get_pending_action(&pending).kind {
                shared::ActionKind::Decrease => c.try_settle_decrease(&p.keeper, &pending),
                _ => c.try_settle_close(&p.keeper, &pending),
            };
        }
        if c.try_get_position(&id).is_err() {
            continue;
        }
        p.observe(1, PRICES[0]);
        if let Ok(Ok(action)) = c.try_create_close(&id, &0) {
            w.actions.push(action);
            p.observe(6, PRICES[0]);
            let _ = c.try_settle_close(&p.keeper, &action);
        }
        let _ = c.try_liquidate_position(&p.keeper, &id);
    }
    w.check(&format!("seed {seed:#x} after unwinding"));
    assert!(w.live_positions().is_empty(), "seed {seed:#x}: every position unwound");
    // Guard against the walk degrading into no-ops after a future change.
    let r = p.router_client();
    let lp_settled = w.lp_requests.iter().filter(|id| r.get_request(id).status == LpRequestStatus::Settled).count();
    assert!(w.max_position_id >= 8, "seed {seed:#x}: only {} positions opened", w.max_position_id);
    assert!(lp_settled >= 3, "seed {seed:#x}: only {lp_settled} LP requests settled");
}

#[test]
fn accounting_reconciles_after_every_step_of_every_seed() {
    for seed in SEEDS {
        walk(seed);
    }
}

/// Opening and closing at an unchanged price can only cost the trader:
/// keeper rewards, fees and borrow all flow away from them, and rounding
/// never favours them.
#[test]
fn a_round_trip_at_the_same_price_never_profits() {
    for (is_long, size, collateral) in [
        (true, usd(10_000), usd(1_000)),
        (false, usd(10_000), usd(1_000)),
        (true, usd(100_000), usd(5_100)),
        (false, usd(73_333), usd(4_321)),
    ] {
        let p = Protocol::new();
        let trader = Address::generate(&p.env);
        p.mint(&trader, usd(100_000));
        let before = p.cash(&trader);
        let id = p.open(&trader, is_long, size, collateral);
        p.observe(defaults::MIN_POSITION_LIFETIME, FILL);
        let close = p.pm().create_close(&id, &0);
        p.observe(6, FILL);
        assert_eq!(p.pm().settle_close(&p.keeper, &close), ActionOutcome::Executed);
        assert!(
            p.cash(&trader) < before,
            "{} {size} at an unchanged price returned {} of {before}",
            if is_long { "long" } else { "short" },
            p.cash(&trader)
        );
        p.assert_conserved("after a same-price round trip");
    }
}
