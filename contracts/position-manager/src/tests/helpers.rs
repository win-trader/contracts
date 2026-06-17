#![cfg(test)]

//! Shared fixture helpers for the position-manager test suite.

use soroban_sdk::{vec, Address, Env, Symbol, Vec};

/// Two mock-oracle price sources updated in lockstep. The oracle-router
/// enforces a quorum floor of 2 sources, so every fixture registers a pair;
/// tests keep the single `set_price` call they always had and both sources
/// move together (the median stays exact and deviation stays zero).
pub struct DualOracle<'a> {
    pub a: mock_oracle::MockOracleClient<'a>,
    pub b: mock_oracle::MockOracleClient<'a>,
}

impl DualOracle<'_> {
    pub fn set_price(&self, symbol: &Symbol, price: &i128) {
        self.a.set_price(symbol, price);
        self.b.set_price(symbol, price);
    }
}

/// Register and initialize two mock oracles. Returns the lockstep client
/// pair and the source list to pass to `set_oracle_sources`.
pub fn register_dual_oracle(env: &Env) -> (DualOracle<'_>, Vec<Address>) {
    let id_a = env.register(mock_oracle::MockOracle, ());
    let a = mock_oracle::MockOracleClient::new(env, &id_a);
    a.initialize();
    let id_b = env.register(mock_oracle::MockOracle, ());
    let b = mock_oracle::MockOracleClient::new(env, &id_b);
    b.initialize();
    let sources = vec![env, id_a, id_b];
    (DualOracle { a, b }, sources)
}
