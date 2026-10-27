//! The PositionManager priced by the in-house `cex-feed` (two CEX publishers,
//! median, quorum of two) instead of the test mock.

mod spec_harness;

use spec_harness::*;

use cex_feed::{CexFeedContract, CexFeedContractClient, FeedConfig, SourcePrice};
use shared::constants::ROLE_ORACLE;
use shared::{defaults, ActionOutcome};
use soroban_sdk::{testutils::Address as _, vec, Address, Symbol};

struct Feed<'a> {
    client: CexFeedContractClient<'a>,
    binance: Address,
    kucoin: Address,
}

impl Feed<'_> {
    fn publish(&self, p: &Protocol, publisher: &Address, price: i128) {
        let batch = vec![&p.env, SourcePrice { symbol: p.market.clone(), price }];
        self.client.set_prices(publisher, &batch);
    }
}

fn install(p: &Protocol) -> Feed<'static> {
    let oracle = Address::generate(&p.env);
    config_manager::ConfigManagerClient::new(&p.env, &p.config_manager).grant_role(
        &p.admin,
        &Symbol::new(&p.env, ROLE_ORACLE),
        &oracle,
    );
    let binance = Address::generate(&p.env);
    let kucoin = Address::generate(&p.env);
    let id = p.env.register(
        CexFeedContract,
        (
            p.config_manager.clone(),
            86_400u64,
            vec![&p.env, binance.clone(), kucoin.clone()],
            FeedConfig {
                max_source_age_seconds: 30,
                max_deviation_bps: 100,
                min_sources: 2,
            },
        ),
    );
    p.gov().propose_price_feed(&oracle, &id);
    p.observe(defaults::CONFIG_TIMELOCK_SECONDS, FILL);
    p.gov().apply_price_feed(&p.keeper);
    Feed {
        client: CexFeedContractClient::new(&p.env, &id),
        binance,
        kucoin,
    }
}

#[test]
fn a_fill_waits_for_both_publishers_to_observe_after_the_commit() {
    let p = Protocol::new();
    let feed = install(&p);
    let c = p.pm();
    feed.publish(&p, &feed.binance, usd(49_990));
    feed.publish(&p, &feed.kucoin, usd(50_010));

    let mut order = p.open_payload(300);
    order.acceptable_price = 0;
    let action = c.create_market_open(&p.trader, &p.market, &order);
    p.wait(6);
    feed.publish(&p, &feed.binance, usd(49_995));
    // KuCoin's stamp still predates the commit, so the median does too.
    assert_eq!(
        c.settle_market_open(&p.keeper, &action),
        ActionOutcome::NotReady
    );

    p.wait(1);
    feed.publish(&p, &feed.kucoin, usd(50_005));
    assert_eq!(
        c.settle_market_open(&p.keeper, &action),
        ActionOutcome::Executed
    );
    assert_eq!(
        c.get_position(&1).base_exposure,
        order.size * DOLLAR / usd(50_000)
    );
}

#[test]
fn a_silent_publisher_stops_settlement_instead_of_pricing_from_one_source() {
    let p = Protocol::new();
    let feed = install(&p);
    let c = p.pm();
    feed.publish(&p, &feed.binance, usd(50_000));
    feed.publish(&p, &feed.kucoin, usd(50_000));
    let mut order = p.open_payload(300);
    order.acceptable_price = 0;
    let action = c.create_market_open(&p.trader, &p.market, &order);
    p.wait(31);
    feed.publish(&p, &feed.binance, usd(50_000));
    assert!(c.try_settle_market_open(&p.keeper, &action).is_err());
}
