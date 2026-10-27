use cex_feed::{CexFeedContract, CexFeedContractClient, CexFeedError, FeedConfig, SourcePrice};
use config_manager::{ConfigManagerClient, ConfigManagerContract};
use shared::{Asset, PriceData};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{vec, Address, Env, Symbol, Vec};

const TIMELOCK: u64 = 86_400;
const START: u64 = 1_000_000;

struct Setup {
    env: Env,
    feed: CexFeedContractClient<'static>,
    admin: Address,
    oracle: Address,
    binance: Address,
    kucoin: Address,
}

fn config(min_sources: u32) -> FeedConfig {
    FeedConfig {
        max_source_age_seconds: 30,
        max_deviation_bps: 100,
        min_sources,
    }
}

fn setup(min_sources: u32) -> Setup {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(START);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let cm_id = env.register(ConfigManagerContract, (admin.clone(),));
    ConfigManagerClient::new(&env, &cm_id).grant_role(
        &admin,
        &Symbol::new(&env, "ORACLE"),
        &oracle,
    );
    let binance = Address::generate(&env);
    let kucoin = Address::generate(&env);
    let publishers = vec![&env, binance.clone(), kucoin.clone()];
    let id = env.register(
        CexFeedContract,
        (cm_id, TIMELOCK, publishers, config(min_sources)),
    );
    let feed = CexFeedContractClient::new(&env, &id);
    Setup {
        env,
        feed,
        admin,
        oracle,
        binance,
        kucoin,
    }
}

impl Setup {
    fn btc(&self) -> Symbol {
        Symbol::new(&self.env, "BTCUSD")
    }
    fn asset(&self) -> Asset {
        Asset::Other(self.btc())
    }
    fn advance(&self, seconds: u64) {
        let now = self.env.ledger().timestamp();
        self.env.ledger().set_timestamp(now + seconds);
    }
    fn publish(&self, publisher: &Address, price: i128) {
        self.feed
            .set_prices(publisher, &one(&self.env, &self.btc(), price));
    }
}

fn one(env: &Env, symbol: &Symbol, price: i128) -> Vec<SourcePrice> {
    vec![
        env,
        SourcePrice {
            symbol: symbol.clone(),
            price,
        },
    ]
}

fn error<T, E>(result: Result<T, Result<E, soroban_sdk::InvokeError>>) -> CexFeedError
where
    E: core::fmt::Debug,
    CexFeedError: TryFrom<E>,
    <CexFeedError as TryFrom<E>>::Error: core::fmt::Debug,
{
    match result {
        Err(Ok(e)) => CexFeedError::try_from(e).unwrap(),
        _ => panic!("expected a contract error"),
    }
}

#[test]
fn speaks_sep40_at_seven_decimals() {
    let s = setup(2);
    assert_eq!(s.feed.decimals(), 7);
    assert_eq!(s.feed.lastprice(&s.asset()), None);
    assert_eq!(s.feed.lastprice(&Asset::Stellar(s.admin.clone())), None);
}

#[test]
fn two_sources_average_and_carry_the_oldest_stamp() {
    let s = setup(2);
    s.publish(&s.binance, 100_000_0000000);
    s.advance(5);
    s.publish(&s.kucoin, 100_100_0000000);
    assert_eq!(
        s.feed.lastprice(&s.asset()),
        Some(PriceData {
            price: 100_050_0000000,
            timestamp: START
        })
    );
}

#[test]
fn quorum_counts_only_fresh_sources() {
    let s = setup(2);
    s.publish(&s.binance, 100_000_0000000);
    s.publish(&s.kucoin, 100_000_0000000);
    s.advance(31);
    s.publish(&s.binance, 101_000_0000000);
    // KuCoin is 31s old against a 30s limit.
    assert_eq!(s.feed.lastprice(&s.asset()), None);
    s.publish(&s.kucoin, 101_000_0000000);
    assert_eq!(s.feed.lastprice(&s.asset()).unwrap().price, 101_000_0000000);
}

#[test]
fn a_source_exactly_at_the_age_limit_is_fresh() {
    let s = setup(1);
    s.publish(&s.binance, 50_0000000);
    s.advance(30);
    assert_eq!(s.feed.lastprice(&s.asset()).unwrap().timestamp, START);
    s.advance(1);
    assert_eq!(s.feed.lastprice(&s.asset()), None);
}

#[test]
fn one_source_quorum_uses_whichever_is_fresh() {
    let s = setup(1);
    s.publish(&s.kucoin, 42_0000000);
    assert_eq!(s.feed.lastprice(&s.asset()).unwrap().price, 42_0000000);
}

#[test]
fn divergent_sources_return_none() {
    let s = setup(2);
    // Median 100.5: each side is 49.75 bps away, inside 100.
    s.publish(&s.binance, 100_0000000);
    s.publish(&s.kucoin, 101_0000000);
    assert!(s.feed.lastprice(&s.asset()).is_some());
    // Median 101: each side is 99 bps away, inside 100.
    s.publish(&s.kucoin, 102_0000000);
    assert!(s.feed.lastprice(&s.asset()).is_some());
    // Median 101.03: each side is 101 bps away.
    s.publish(&s.kucoin, 102_0600000);
    assert_eq!(s.feed.lastprice(&s.asset()), None);
}

#[test]
fn symbols_are_independent() {
    let s = setup(1);
    s.publish(&s.binance, 100_0000000);
    let eth = Asset::Other(Symbol::new(&s.env, "ETHUSD"));
    assert_eq!(s.feed.lastprice(&eth), None);
}

#[test]
fn only_publishers_publish_positive_bounded_prices() {
    let s = setup(1);
    let stranger = Address::generate(&s.env);
    let btc = s.btc();
    assert_eq!(
        error(s.feed.try_set_prices(&stranger, &one(&s.env, &btc, 1))),
        CexFeedError::NotPublisher
    );
    assert_eq!(
        error(s.feed.try_set_prices(&s.binance, &one(&s.env, &btc, 0))),
        CexFeedError::InvalidPrice
    );
    assert_eq!(
        error(s.feed.try_set_prices(&s.binance, &one(&s.env, &btc, -1))),
        CexFeedError::InvalidPrice
    );
    assert_eq!(
        error(
            s.feed
                .try_set_prices(&s.binance, &one(&s.env, &btc, i128::MAX / 10_000))
        ),
        CexFeedError::InvalidPrice
    );
    assert_eq!(s.feed.source_price(&s.binance, &btc), None);
}

#[test]
fn set_prices_requires_the_publishers_signature() {
    let s = setup(1);
    s.env.set_auths(&[]);
    let btc = s.btc();
    assert!(s
        .feed
        .try_set_prices(&s.binance, &one(&s.env, &btc, 1))
        .is_err());
}

#[test]
fn constructor_rejects_invalid_sources() {
    let env = Env::default();
    let cm = Address::generate(&env);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let cases: [(u64, Vec<Address>, FeedConfig); 7] = [
        (3_600, vec![&env, a.clone()], config(1)),
        (TIMELOCK, Vec::new(&env), config(1)),
        (TIMELOCK, vec![&env, a.clone(), a.clone()], config(1)),
        (TIMELOCK, vec![&env, a.clone(), b.clone()], config(3)),
        (TIMELOCK, vec![&env, a.clone()], config(0)),
        (
            TIMELOCK,
            vec![&env, a.clone()],
            FeedConfig {
                max_source_age_seconds: 3_601,
                ..config(1)
            },
        ),
        (
            TIMELOCK,
            vec![&env, a.clone()],
            FeedConfig {
                max_deviation_bps: 0,
                ..config(1)
            },
        ),
    ];
    for (timelock, publishers, config) in cases {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            env.register(CexFeedContract, (cm.clone(), timelock, publishers, config));
        }));
        assert!(result.is_err());
    }
}

#[test]
fn publisher_set_changes_wait_for_the_timelock() {
    let s = setup(2);
    let fresh = Address::generate(&s.env);
    let next = vec![&s.env, s.binance.clone(), fresh.clone()];
    s.feed.propose_sources(&s.oracle, &next, &config(2));
    assert_eq!(
        s.feed.pending_sources().unwrap().effective_at,
        START + TIMELOCK
    );
    assert_eq!(
        error(s.feed.try_apply_sources(&s.admin)),
        CexFeedError::SourcesTimelockNotElapsed
    );
    s.advance(TIMELOCK);
    s.feed.apply_sources(&s.admin);
    assert_eq!(s.feed.publishers(), next);
    assert_eq!(s.feed.pending_sources(), None);
    // The removed publisher no longer counts, even with a fresh price.
    s.publish(&s.binance, 100_0000000);
    assert_eq!(
        error(
            s.feed
                .try_set_prices(&s.kucoin, &one(&s.env, &s.btc(), 100_0000000))
        ),
        CexFeedError::NotPublisher
    );
    assert_eq!(s.feed.lastprice(&s.asset()), None);
    s.publish(&fresh, 100_0000000);
    assert!(s.feed.lastprice(&s.asset()).is_some());
}

#[test]
fn a_due_proposal_lapses_after_one_more_timelock() {
    let s = setup(2);
    s.feed
        .propose_sources(&s.oracle, &vec![&s.env, s.binance.clone()], &config(1));
    s.advance(2 * TIMELOCK + 1);
    assert_eq!(
        error(s.feed.try_apply_sources(&s.admin)),
        CexFeedError::SourcesProposalExpired
    );
}

#[test]
fn only_oracle_proposes_and_admin_can_cancel() {
    let s = setup(2);
    let publishers = vec![&s.env, s.binance.clone()];
    assert_eq!(
        error(
            s.feed
                .try_propose_sources(&s.admin, &publishers, &config(1))
        ),
        CexFeedError::Unauthorized
    );
    s.feed.propose_sources(&s.oracle, &publishers, &config(1));
    let stranger = Address::generate(&s.env);
    assert_eq!(
        error(s.feed.try_cancel_sources(&stranger)),
        CexFeedError::Unauthorized
    );
    s.feed.cancel_sources(&s.admin);
    assert_eq!(s.feed.pending_sources(), None);
    assert_eq!(
        error(s.feed.try_cancel_sources(&s.oracle)),
        CexFeedError::NoPendingSources
    );
    assert_eq!(
        error(s.feed.try_apply_sources(&s.admin)),
        CexFeedError::NoPendingSources
    );
}

#[test]
fn proposals_are_validated() {
    let s = setup(2);
    assert_eq!(
        error(
            s.feed
                .try_propose_sources(&s.oracle, &vec![&s.env, s.binance.clone()], &config(2))
        ),
        CexFeedError::InvalidConfig
    );
}

#[test]
fn removing_a_publisher_is_immediate_and_fails_closed() {
    let s = setup(2);
    s.publish(&s.binance, 100_0000000);
    s.publish(&s.kucoin, 100_0000000);
    assert!(s.feed.lastprice(&s.asset()).is_some());
    s.feed.remove_publisher(&s.admin, &s.kucoin);
    assert_eq!(s.feed.publishers(), vec![&s.env, s.binance.clone()]);
    // One publisher left against a quorum of two: no price at all.
    assert_eq!(s.feed.lastprice(&s.asset()), None);
    assert_eq!(
        error(s.feed.try_remove_publisher(&s.oracle, &s.kucoin)),
        CexFeedError::NotPublisher
    );
    let stranger = Address::generate(&s.env);
    assert_eq!(
        error(s.feed.try_remove_publisher(&stranger, &s.binance)),
        CexFeedError::Unauthorized
    );
}

#[test]
fn three_sources_take_the_middle_one() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(START);
    let admin = Address::generate(&env);
    let cm = env.register(ConfigManagerContract, (admin,));
    let p: [Address; 3] = core::array::from_fn(|_| Address::generate(&env));
    let id = env.register(
        CexFeedContract,
        (
            cm,
            TIMELOCK,
            vec![&env, p[0].clone(), p[1].clone(), p[2].clone()],
            config(2),
        ),
    );
    let feed = CexFeedContractClient::new(&env, &id);
    let btc = Symbol::new(&env, "BTCUSD");
    feed.set_prices(&p[0], &one(&env, &btc, 100_0000000));
    feed.set_prices(&p[1], &one(&env, &btc, 100_5000000));
    feed.set_prices(&p[2], &one(&env, &btc, 99_9000000));
    assert_eq!(
        feed.lastprice(&Asset::Other(btc)).unwrap().price,
        100_0000000
    );
}

#[test]
fn one_call_publishes_every_symbol_at_the_same_stamp() {
    let s = setup(1);
    let eth = Symbol::new(&s.env, "ETHUSD");
    let batch = vec![
        &s.env,
        SourcePrice {
            symbol: s.btc(),
            price: 100_000_0000000,
        },
        SourcePrice {
            symbol: eth.clone(),
            price: 3_000_0000000,
        },
    ];
    s.feed.set_prices(&s.binance, &batch);
    assert_eq!(
        s.feed.lastprice(&Asset::Other(eth)),
        Some(PriceData {
            price: 3_000_0000000,
            timestamp: START
        })
    );
    assert_eq!(s.feed.lastprice(&s.asset()).unwrap().price, 100_000_0000000);
}

#[test]
fn one_bad_price_rejects_the_whole_batch() {
    let s = setup(1);
    let batch = vec![
        &s.env,
        SourcePrice {
            symbol: s.btc(),
            price: 100_0000000,
        },
        SourcePrice {
            symbol: Symbol::new(&s.env, "ETHUSD"),
            price: 0,
        },
    ];
    assert_eq!(
        error(s.feed.try_set_prices(&s.binance, &batch)),
        CexFeedError::InvalidPrice
    );
    assert_eq!(s.feed.source_price(&s.binance, &s.btc()), None);
}

#[test]
fn batches_are_non_empty_and_bounded() {
    let s = setup(1);
    assert_eq!(
        error(s.feed.try_set_prices(&s.binance, &Vec::new(&s.env))),
        CexFeedError::InvalidPrice
    );
    let mut batch = Vec::new(&s.env);
    for _ in 0..17 {
        batch.push_back(SourcePrice {
            symbol: s.btc(),
            price: 1,
        });
    }
    assert_eq!(
        error(s.feed.try_set_prices(&s.binance, &batch)),
        CexFeedError::InvalidPrice
    );
    batch.pop_back();
    s.feed.set_prices(&s.binance, &batch);
}
