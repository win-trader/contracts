use soroban_sdk::contracttype;

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeSource {
    Opening,
    Closing,
    Borrow,
}
