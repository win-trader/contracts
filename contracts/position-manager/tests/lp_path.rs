//! Integration coverage for the §7.17 LP request path.
//!
//! The properties worth proving here are about **where the resolve reward
//! comes from**, because each of the three sources is chosen for a different
//! reason and getting one wrong is invisible in the happy path:
//!
//! - a deposit pays it from its own asset escrow *before* conversion, so no
//!   share is minted against value that went to the executor;
//! - a successful withdrawal pays it from the assets it releases, *after*
//!   every capacity and health check has passed on the full amount; and
//! - a failed withdrawal pays nothing at all, because its escrow is shares
//!   and taking the reward in shares would confiscate part of an LP's stake
//!   for an outcome they did not cause.

mod common;

use common::{global_config, World, PRICE};
use shared::{
    defaults, LpRequestStatus, RequestRouterClient, SettlementStatus, VaultClient,
};
use soroban_sdk::{testutils::Address as _, Address};

impl World {
    fn router(&self) -> RequestRouterClient<'_> {
        RequestRouterClient::new(&self.env, &self.router)
    }

    fn vault_client(&self) -> VaultClient<'_> {
        VaultClient::new(&self.env, &self.vault)
    }

    /// LP shares are an ordinary fungible token issued by the vault.
    fn shares(&self, who: &Address) -> i128 {
        soroban_sdk::token::Client::new(&self.env, &self.vault).balance(who)
    }
}

const DEPOSIT: i128 = 500_000_0000;

/// §7.17 — the reward is deducted **before** conversion, so the depositor
/// mints shares for the assets that actually reach the vault.
#[test]
fn a_deposit_pays_its_reward_from_escrow_before_conversion() {
    let w = World::new();
    let lp = Address::generate(&w.env);
    let executor = Address::generate(&w.env);
    w.mint(&lp, DEPOSIT);

    let supply_before = w.vault_client().total_share_supply();
    let nav_before = w.vault_client().accounting_snapshot().vault_nav;
    let id = w.router().request_deposit(&lp, &DEPOSIT);
    assert_eq!(w.balance(&lp), 0, "the escrow leaves the LP at creation");

    // §7.17 — not resolvable until the delay elapses, and a premature call
    // is a return rather than a revert: an LP request has no `Expired`
    // outcome, so the head simply stays pending.
    let early = w.router().resolve_next(&executor);
    assert_eq!(early.status, SettlementStatus::NotReady);
    assert_eq!(early.reward, 0);
    assert_eq!(
        w.router().get_request(&id).status,
        LpRequestStatus::Pending,
        "and the request is untouched"
    );

    w.observe(defaults::LP_REQUEST_DELAY_LOCAL, PRICE);
    let result = w.router().resolve_next(&executor);

    let reward = global_config().keeper_rewards.lp_resolve;
    assert_eq!(result.status, SettlementStatus::Settled);
    assert_eq!(result.reward, reward);
    assert_eq!(w.balance(&executor), reward, "paid out of the asset escrow");

    // The minted shares correspond to `DEPOSIT - reward`, not to `DEPOSIT`.
    let expected = (DEPOSIT - reward) * (supply_before + 1_000_000) / (nav_before + 1);
    assert_eq!(w.shares(&lp), expected);
    assert_eq!(result.amount, expected);
}

/// §7.17 — a successful withdrawal pays the reward out of the assets it
/// releases, after the gates have passed on the full amount.
#[test]
fn a_successful_withdrawal_pays_its_reward_from_the_assets_it_releases() {
    let w = World::new();
    let lp = Address::generate(&w.env);
    let executor = Address::generate(&w.env);
    w.mint(&lp, DEPOSIT);
    w.router().request_deposit(&lp, &DEPOSIT);
    w.observe(defaults::LP_REQUEST_DELAY_LOCAL, PRICE);
    w.router().resolve_next(&executor);

    let shares = w.shares(&lp);
    let executor_before = w.balance(&executor);
    w.router().request_withdrawal(&lp, &shares);
    assert_eq!(w.shares(&lp), 0, "the shares are escrowed at creation");

    w.observe(defaults::LP_REQUEST_DELAY_LOCAL, PRICE);
    let result = w.router().resolve_next(&executor);

    let reward = global_config().keeper_rewards.lp_resolve;
    assert_eq!(result.status, SettlementStatus::Settled);
    assert_eq!(result.reward, reward);
    assert_eq!(w.balance(&executor), executor_before + reward);
    assert_eq!(
        w.balance(&lp),
        result.amount - reward,
        "the LP receives the released assets net of the reward"
    );
    assert_eq!(w.vault_client().total_share_supply(), 0, "shares are burned");
}

/// §7.17 — a failed withdrawal pays **no** reward and returns the complete
/// share escrow. Its escrow is shares, not cash.
///
/// The gate used here is the real one: open exposure raises the required
/// risk backing, so free LP capital no longer covers the full stake.
#[test]
fn a_failed_withdrawal_pays_no_reward_and_returns_every_share() {
    let w = World::new();
    let c = w.client();
    let lp = Address::generate(&w.env);
    let executor = Address::generate(&w.env);
    w.mint(&lp, DEPOSIT);
    w.router().request_deposit(&lp, &DEPOSIT);
    w.observe(defaults::LP_REQUEST_DELAY_LOCAL, PRICE);
    w.router().resolve_next(&executor);
    let shares = w.shares(&lp);
    let executor_before = w.balance(&executor);

    // Put enough risk on the book that free capital cannot cover the stake.
    let equity = c.accounting_snapshot(&w.physical()).cash_lp_equity;
    let size = equity * 8;
    let collateral = size / 10;
    w.mint(&w.trader, collateral * 2);
    let mut request = w.request(collateral, 0, 120);
    request.size = size;
    request.submitted_collateral = collateral;
    let action_id = c.create_market_open(&w.trader, &w.market, &request);
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &action_id);

    let id = w.router().request_withdrawal(&lp, &shares);
    w.observe(defaults::LP_REQUEST_DELAY_LOCAL, PRICE);
    let result = w.router().resolve_next(&executor);

    assert_eq!(result.status, SettlementStatus::Failed);
    assert_eq!(result.reward, 0, "a failed withdrawal releases no assets");
    assert_eq!(w.balance(&executor), executor_before);
    assert_eq!(w.shares(&lp), shares, "the complete share escrow comes back");
    assert_eq!(w.router().get_request(&id).status, LpRequestStatus::Failed);
    // §7.17 — failing rather than reverting is what keeps the queue moving:
    // only the head is resolvable, so a `require` would block every LP
    // behind an unsatisfiable request for as long as the condition held.
    assert_eq!(w.router().next_request_to_resolve(), id + 1);
}

/// §7.17 — a failed *deposit* still pays the executor, out of its escrow,
/// and refunds only the remainder. The two failure paths differ because
/// their escrows are different kinds of thing.
///
/// The failure used here is the vault being short of its claims — a real
/// resolution gate. A pause is not one: while paused, requests wait rather
/// than fail (§12.2).
#[test]
fn a_failed_deposit_still_pays_its_reward_and_refunds_the_remainder() {
    let w = World::new();
    let c = w.client();
    let lp = Address::generate(&w.env);
    let executor = Address::generate(&w.env);
    w.mint(&lp, DEPOSIT);
    let id = w.router().request_deposit(&lp, &DEPOSIT);

    // A position's collateral is a claim on vault cash; burn every unit of
    // LP equity and one more, and the vault is short of what it owes.
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &action_id);
    let equity = c.accounting_snapshot(&w.physical()).cash_lp_equity;
    mock_token::MockTokenClient::new(&w.env, &w.token).burn(&w.vault, &(equity + 1));
    assert!(c.accounting_snapshot(&w.physical()).cash_shortfall > 0);

    w.observe(defaults::LP_REQUEST_DELAY_LOCAL, PRICE);
    let result = w.router().resolve_next(&executor);

    let reward = global_config().keeper_rewards.lp_resolve;
    assert_eq!(result.status, SettlementStatus::Failed);
    assert_eq!(result.reward, reward);
    assert_eq!(w.balance(&executor), reward);
    assert_eq!(w.balance(&lp), DEPOSIT - reward, "the remainder is refunded");
    assert_eq!(w.shares(&lp), 0, "and no shares were minted");
    assert_eq!(w.router().get_request(&id).status, LpRequestStatus::Failed);
}
