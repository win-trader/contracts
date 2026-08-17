use soroban_sdk::contracttype;

/// Why a position left the book.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseReason {
    /// The owner decreased to zero.
    Trader,
    /// A keeper or third party liquidated an unhealthy position.
    Liquidation,
    /// Funded auto-deleveraging on an ADL/hard-cap side.
    Deleverage,
    /// A take-profit or stop-loss trigger executed.
    Order,
}
