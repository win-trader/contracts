pub use interfaces::{MarketInfo, Position};

/// Reason a Position Close was triggered — the four Close kinds defined in
/// CONTEXT.md. Determines fee distribution and the routing of any TP/SL
/// execution-fee escrow.
pub enum CloseType {
    User,
    OrderExecution,
    Liquidation,
    Deleverage,
}
