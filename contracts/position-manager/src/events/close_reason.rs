use soroban_sdk::contracttype;

/// Why a position left the book. §12.6 requires the terminal cause on the
/// close result, and the four forced/voluntary paths are distinguishable
/// because they differ in what they pay: a voluntary close and both triggers
/// charge a closing fee, liquidation and ADL charge none.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseReason {
    /// The owner's committed close settled (§7.10).
    VoluntaryClose,
    /// An attached take-profit executed (§7.11).
    TakeProfit,
    /// An attached stop-loss executed (§7.12).
    StopLoss,
    /// The position fell to its liquidation threshold (§7.13).
    Liquidation,
    /// Forced deleveraging on an `ADL` or `HardCap` side (§7.14).
    Adl,
}
