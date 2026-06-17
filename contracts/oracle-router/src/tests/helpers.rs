//! Test helpers for OracleRouter contract tests.
//!
//! Provides `deploy`, `deploy_initialized`, and `deploy_with_config_manager`
//! convenience functions that mirror the pattern established in the
//! config-manager test suite.
//!
//! The router is constructed atomically with deploy: its `__constructor` takes
//! the linked ConfigManager address as its only argument, so registration
//! itself binds the router. There is no separate `initialize` entrypoint and
//! the router stores no admin of its own — every role check cross-calls the
//! linked ConfigManager.

use soroban_sdk::{testutils::Address as _, vec, Address, Env, Symbol};

use crate::{OracleConfig, OracleRouterClient, OracleRouterContract};

#[cfg(test)]
use config_manager::{ConfigManagerClient, ConfigManagerContract};

#[cfg(test)]
use mock_oracle::{MockOracle, MockOracleClient};

/// Register the OracleRouter contract with a freshly generated ConfigManager
/// address as its constructor argument and return the client.
///
/// Because construction is atomic with deploy, the returned client is already
/// fully constructed (the `Initialized` flag is set and the ConfigManager
/// address is bound). The generated config_manager address is a raw address —
/// not a deployed contract — which is sufficient for tests that never trigger
/// a cross-call into it (e.g. auth-stripping tests that fail at
/// `require_auth()` first).
pub fn deploy(env: &Env) -> OracleRouterClient<'_> {
    let config_manager = Address::generate(env);
    let contract_id = env.register(OracleRouterContract, (config_manager,));
    OracleRouterClient::new(env, &contract_id)
}

/// Register the OracleRouter contract with a freshly generated config_manager
/// address as the constructor argument, and return both the client and the
/// address that was bound as the ConfigManager.
///
/// The config_manager address is a raw generated address — not a deployed
/// contract — because the constructor only stores the address in instance
/// storage; it does not cross-call it.
pub fn deploy_initialized(env: &Env) -> (OracleRouterClient<'_>, Address) {
    let config_manager = Address::generate(env);
    let contract_id = env.register(OracleRouterContract, (config_manager.clone(),));
    let client = OracleRouterClient::new(env, &contract_id);
    (client, config_manager)
}

/// Deploy a real ConfigManager contract and a real OracleRouter contract,
/// wire them together, and return all three handles needed for cross-contract
/// admin-auth tests.
///
/// Deployment sequence:
///   1. Register ConfigManager with `admin` as its constructor argument, which
///      grants the `"ADMIN"` role to `admin` in ConfigManager's storage.
///   2. Register OracleRouter with the ConfigManager address as its constructor
///      argument, which binds the ConfigManager in the router's instance
///      storage.
///
/// Returns `(oracle_client, cm_client, admin)`.
#[cfg(test)]
pub fn deploy_with_config_manager(
    env: &Env,
) -> (OracleRouterClient<'_>, ConfigManagerClient<'_>, Address) {
    // 1. Deploy ConfigManager — constructor grants DEFAULT_ADMIN ("ADMIN") to admin.
    let admin = Address::generate(env);
    let cm_id = env.register(ConfigManagerContract, (admin.clone(),));
    let cm = ConfigManagerClient::new(env, &cm_id);

    // 2. Deploy OracleRouter linked to the ConfigManager via its constructor.
    let oracle_id = env.register(OracleRouterContract, (cm_id.clone(),));
    let oracle = OracleRouterClient::new(env, &oracle_id);

    (oracle, cm, admin)
}

// ---------------------------------------------------------------------------
// Data fixture helpers
// ---------------------------------------------------------------------------

/// Returns a canonical valid OracleConfig suitable for use across all tests
/// that require a non-zero configuration. `min_required_sources` is at the
/// quorum floor (2) so a single source can never set the median.
pub fn valid_oracle_config() -> OracleConfig {
    OracleConfig {
        max_deviation_bps: 100,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    }
}

/// Role symbol helper — returns the "ADMIN" symbol used by ConfigManager's
/// `has_role` check.  Keeps test files free of raw string literals.
pub fn role_admin(env: &Env) -> Symbol {
    Symbol::new(env, "ADMIN")
}

/// Role symbol helper — returns the "UPGRADER" symbol used by ConfigManager's
/// `has_role` check for upgrade authorization.
pub fn role_upgrader(env: &Env) -> Symbol {
    Symbol::new(env, "UPGRADER")
}

// ---------------------------------------------------------------------------
// Upgrade helpers (test-only)
// ---------------------------------------------------------------------------

/// Deploy OracleRouter + ConfigManager, then grant the UPGRADER role to admin.
///
/// This extends `deploy_with_config_manager` by calling:
///   `cm.grant_role(&admin, &Symbol::new(env, "UPGRADER"), &admin)`
///
/// Returns `(oracle_client, cm_client, admin)` where `admin` holds both
/// the DEFAULT_ADMIN ("ADMIN") role and the UPGRADER role.
#[cfg(test)]
pub fn deploy_with_upgrader(
    env: &Env,
) -> (OracleRouterClient<'_>, ConfigManagerClient<'_>, Address) {
    let (oracle, cm, admin) = deploy_with_config_manager(env);

    // Grant the UPGRADER role to admin via the admin's own authority.
    let upgrader_role = role_upgrader(env);
    cm.grant_role(&admin, &upgrader_role, &admin);

    (oracle, cm, admin)
}

// ---------------------------------------------------------------------------
// Mock oracle helpers (test-only)
// ---------------------------------------------------------------------------

/// Deploy a MockOracle contract and return its client.
///
/// The returned client exposes `set_price(symbol, price)` and `last_update(symbol)`
/// so individual tests can control the exact price and freshness of this source.
#[cfg(test)]
pub fn deploy_mock_oracle(env: &Env) -> MockOracleClient<'_> {
    let id = env.register(MockOracle, ());
    MockOracleClient::new(env, &id)
}

/// Two mock oracle sources driven in lockstep. The quorum floor requires at
/// least two valid sources, so the single-source `get_price` tests register a
/// pair and move both together: `set_price` fans out to both, keeping the
/// median exact and the deviation zero. The pair models "one logical feed"
/// for tests that don't care about source disagreement.
#[cfg(test)]
pub struct DualMock<'a> {
    pub a: MockOracleClient<'a>,
    pub b: MockOracleClient<'a>,
}

#[cfg(test)]
impl DualMock<'_> {
    pub fn set_price(&self, symbol: &Symbol, price: &i128) {
        self.a.set_price(symbol, price);
        self.b.set_price(symbol, price);
    }
}

/// Full setup for `get_price` tests.
///
/// Deployment sequence:
///   1. Calls `deploy_with_config_manager` — gives us an admin + linked CM.
///   2. Sets a valid OracleConfig via `set_oracle_config`:
///        max_deviation_bps = 200, staleness_threshold = 60, cache_duration = 10,
///        min_required_sources = 2 (the quorum floor).
///   3. Deploys TWO MockOracles and registers both as the sources for
///      `Symbol::new(env, "ETH")`. The returned `DualMock` sets both in lockstep.
///
/// Returns `(oracle_client, dual_mock, admin)`.
///
/// Callers MUST call `env.mock_all_auths()` before invoking this helper if
/// they need the admin setup calls to succeed.
#[cfg(test)]
pub fn deploy_with_price_feed(env: &Env) -> (OracleRouterClient<'_>, DualMock<'_>, Address) {
    let (oracle, _cm, admin) = deploy_with_config_manager(env);

    let config = OracleConfig {
        max_deviation_bps: 200,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let a = deploy_mock_oracle(env);
    let b = deploy_mock_oracle(env);
    let eth = Symbol::new(env, "ETH");
    let sources = vec![env, a.address.clone(), b.address.clone()];
    oracle.set_oracle_sources(&admin, &eth, &sources);

    (oracle, DualMock { a, b }, admin)
}
