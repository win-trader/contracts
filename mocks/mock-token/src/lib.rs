//! Mock SEP-41 fungible token for use in integration tests.
//!
//! Exposes a capped public faucet (`mint`), admin seeding (`admin_mint`), and
//! optional transfer restrictions for deployed testnet environments.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Env,
    MuxedAddress, String,
};
use stellar_tokens::fungible::{burnable::FungibleBurnable, Base, FungibleToken};

const PUBLIC_MINT_CAP_USD: i128 = 5_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MockTokenError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    MintCapExceeded = 3,
    TransferRestricted = 4,
}

#[contracttype]
enum MockTokenDataKey {
    Admin,
    RestrictionsActive,
    ProtocolContract(Address),
    PublicMinted(Address),
    PublicMintCapUsd,
}

#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    /// Deploy and configure the mock token.
    pub fn initialize(env: Env, admin: Address, decimals: u32, name: String, symbol: String) {
        if env.storage().instance().has(&MockTokenDataKey::Admin) {
            panic_with_error!(env, MockTokenError::AlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&MockTokenDataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&MockTokenDataKey::RestrictionsActive, &false);
        Base::set_metadata(&env, decimals, name, symbol);
    }

    /// Public faucet mint. Once protocol restrictions are activated, each
    /// address gets one mint of up to 5,000 test USDC.
    pub fn mint(env: Env, to: Address, amount: i128) {
        if restrictions_active(&env) {
            to.require_auth();
            add_public_mint(&env, &to, amount);
        }
        Base::mint(&env, &to, amount);
    }

    /// Admin-only mint for deployment seeding and simulations. This bypasses
    /// the public faucet cap and must not be exposed as a user faucet path.
    /// The credit consumes the recipient's one-shot faucet claim: a wallet
    /// funded directly must never also claim the public mint on top.
    pub fn admin_mint(env: Env, admin: Address, to: Address, amount: i128) {
        require_admin(&env, &admin);
        set_public_minted(&env, &to, public_minted(&env, &to) + amount);
        Base::mint(&env, &to, amount);
    }

    /// Activate protocol-only transfer mode and mark the two perps contracts
    /// that may receive from users and pay back to users.
    pub fn configure_protocol(env: Env, admin: Address, vault: Address, position_manager: Address) {
        require_admin(&env, &admin);
        set_protocol_contract(&env, &vault, true);
        set_protocol_contract(&env, &position_manager, true);
        env.storage()
            .instance()
            .set(&MockTokenDataKey::RestrictionsActive, &true);
    }

    pub fn set_protocol_contract(env: Env, admin: Address, contract: Address, allowed: bool) {
        require_admin(&env, &admin);
        set_protocol_contract(&env, &contract, allowed);
        env.storage()
            .instance()
            .set(&MockTokenDataKey::RestrictionsActive, &true);
    }

    pub fn restrictions_active(env: Env) -> bool {
        restrictions_active(&env)
    }

    pub fn is_protocol_contract(env: Env, contract: Address) -> bool {
        is_protocol_contract(&env, &contract)
    }

    pub fn public_minted(env: Env, account: Address) -> i128 {
        public_minted(&env, &account)
    }

    pub fn public_mint_cap(env: Env) -> i128 {
        public_mint_cap(&env)
    }

    /// Admin-set faucet cap in whole USD, so weekly competition stakes can
    /// change without redeploying the token. Already-claimed addresses are
    /// unaffected (the claim is one-shot regardless of amount).
    pub fn set_public_mint_cap(env: Env, admin: Address, cap_usd: i128) {
        require_admin(&env, &admin);
        if cap_usd <= 0 {
            panic_with_error!(env, MockTokenError::MintCapExceeded);
        }
        env.storage()
            .instance()
            .set(&MockTokenDataKey::PublicMintCapUsd, &cap_usd);
    }
}

/// SEP-41 token interface — auto-implemented by OZ Base.
#[contractimpl(contracttrait)]
impl FungibleToken for MockToken {
    type ContractType = Base;

    fn transfer(e: &Env, from: Address, to: MuxedAddress, amount: i128) {
        let to_addr = to.address();
        require_protocol_endpoint(e, &from, &to_addr);
        Base::transfer(e, &from, &to, amount);
    }

    fn transfer_from(e: &Env, spender: Address, from: Address, to: Address, amount: i128) {
        require_protocol_endpoint(e, &from, &to);
        Base::transfer_from(e, &spender, &from, &to, amount);
    }
}

/// Burn support — auto-implemented by OZ FungibleBurnable.
#[contractimpl(contracttrait)]
impl FungibleBurnable for MockToken {}

fn require_admin(env: &Env, admin: &Address) {
    admin.require_auth();
    let stored = env
        .storage()
        .instance()
        .get::<_, Address>(&MockTokenDataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(env, MockTokenError::Unauthorized));
    if stored != *admin {
        panic_with_error!(env, MockTokenError::Unauthorized);
    }
}

fn restrictions_active(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&MockTokenDataKey::RestrictionsActive)
        .unwrap_or(false)
}

fn set_protocol_contract(env: &Env, contract: &Address, allowed: bool) {
    env.storage().instance().set(
        &MockTokenDataKey::ProtocolContract(contract.clone()),
        &allowed,
    );
}

fn is_protocol_contract(env: &Env, contract: &Address) -> bool {
    env.storage()
        .instance()
        .get(&MockTokenDataKey::ProtocolContract(contract.clone()))
        .unwrap_or(false)
}

fn require_protocol_endpoint(env: &Env, from: &Address, to: &Address) {
    if !restrictions_active(env) {
        return;
    }
    if is_protocol_contract(env, from) || is_protocol_contract(env, to) {
        return;
    }
    panic_with_error!(env, MockTokenError::TransferRestricted);
}

fn public_minted(env: &Env, account: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&MockTokenDataKey::PublicMinted(account.clone()))
        .unwrap_or(0)
}

fn set_public_minted(env: &Env, account: &Address, amount: i128) {
    env.storage()
        .persistent()
        .set(&MockTokenDataKey::PublicMinted(account.clone()), &amount);
}

fn public_mint_cap(env: &Env) -> i128 {
    let cap_usd = env
        .storage()
        .instance()
        .get(&MockTokenDataKey::PublicMintCapUsd)
        .unwrap_or(PUBLIC_MINT_CAP_USD);
    cap_usd * 10_i128.pow(Base::decimals(env))
}

fn add_public_mint(env: &Env, account: &Address, amount: i128) {
    // `PublicMinted` doubles as the one-shot claim marker. Checking the stored
    // amount preserves the claimed state for addresses that minted under an
    // earlier contract version, without adding a storage migration.
    if amount <= 0 || amount > public_mint_cap(env) || public_minted(env, account) > 0 {
        panic_with_error!(env, MockTokenError::MintCapExceeded);
    }
    set_public_minted(env, account, amount);
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (
        Env,
        MockTokenClient<'static>,
        Address,
        Address,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let token_id = env.register(MockToken, ());
        let token = MockTokenClient::new(&env, &token_id);
        let admin = Address::generate(&env);
        let vault = Address::generate(&env);
        let pm = Address::generate(&env);
        let user = Address::generate(&env);

        token.initialize(
            &admin,
            &7u32,
            &String::from_str(&env, "Test USD"),
            &String::from_str(&env, "tUSD"),
        );
        token.configure_protocol(&admin, &vault, &pm);

        (
            env,
            unsafe { core::mem::transmute(token) },
            admin,
            vault,
            pm,
            user,
        )
    }

    #[test]
    fn public_mint_is_one_shot_per_address() {
        let (_env, token, _admin, _vault, _pm, user) = setup();
        let cap = token.public_mint_cap();
        let first_mint = cap / 5;

        token.mint(&user, &first_mint);
        assert_eq!(token.balance(&user), first_mint);
        assert_eq!(token.public_minted(&user), first_mint);

        let err = token
            .try_mint(&user, &(cap - first_mint))
            .unwrap_err()
            .unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(MockTokenError::MintCapExceeded as u32)
        );
    }

    #[test]
    fn rejected_amount_does_not_spend_the_one_shot_claim() {
        let (_env, token, _admin, _vault, _pm, user) = setup();
        let cap = token.public_mint_cap();

        let err = token.try_mint(&user, &(cap + 1)).unwrap_err().unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(MockTokenError::MintCapExceeded as u32)
        );
        assert_eq!(token.public_minted(&user), 0);

        token.mint(&user, &cap);
        assert_eq!(token.balance(&user), cap);
        assert_eq!(token.public_minted(&user), cap);
    }

    #[test]
    fn admin_can_lower_the_faucet_cap_without_redeploying() {
        let (_env, token, admin, _vault, _pm, user) = setup();
        token.set_public_mint_cap(&admin, &500);
        let cap = token.public_mint_cap();
        assert_eq!(cap, 500 * 10_i128.pow(7));

        let err = token.try_mint(&user, &(cap + 1)).unwrap_err().unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(MockTokenError::MintCapExceeded as u32)
        );
        token.mint(&user, &cap);
        assert_eq!(token.balance(&user), cap);
    }

    #[test]
    fn admin_mint_bypasses_public_cap_and_consumes_the_faucet_claim() {
        let (_env, token, admin, _vault, _pm, user) = setup();
        let amount = token.public_mint_cap() * 10;

        token.admin_mint(&admin, &user, &amount);

        assert_eq!(token.balance(&user), amount);
        assert_eq!(token.public_minted(&user), amount);

        // A credited wallet can no longer stack a public faucet claim on
        // top of its funding.
        let err = token.try_mint(&user, &1).unwrap_err().unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(MockTokenError::MintCapExceeded as u32)
        );

        // Further admin credits still work and accumulate the record.
        token.admin_mint(&admin, &user, &amount);
        assert_eq!(token.balance(&user), 2 * amount);
        assert_eq!(token.public_minted(&user), 2 * amount);
    }

    #[test]
    fn transfer_requires_protocol_endpoint_when_restricted() {
        let (env, token, admin, vault, pm, user) = setup();
        let other = Address::generate(&env);

        token.admin_mint(&admin, &user, &1_000);

        let err = token
            .try_transfer(&user, &other, &100)
            .unwrap_err()
            .unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(MockTokenError::TransferRestricted as u32)
        );

        token.transfer(&user, &vault, &100);
        assert_eq!(token.balance(&vault), 100);

        token.transfer(&vault, &user, &25);
        assert_eq!(token.balance(&user), 925);

        token.transfer(&user, &pm, &50);
        assert_eq!(token.balance(&pm), 50);
    }

    #[test]
    fn delegated_transfer_also_requires_a_protocol_endpoint() {
        let (env, token, admin, vault, _pm, user) = setup();
        let spender = Address::generate(&env);
        let other = Address::generate(&env);

        token.admin_mint(&admin, &user, &1_000);
        token.approve(&user, &spender, &1_000, &100);

        let err = token
            .try_transfer_from(&spender, &user, &other, &100)
            .unwrap_err()
            .unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(MockTokenError::TransferRestricted as u32)
        );

        token.transfer_from(&spender, &user, &vault, &100);
        assert_eq!(token.balance(&vault), 100);
        assert_eq!(token.balance(&user), 900);
    }
}
