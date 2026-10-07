use soroban_sdk::contracttype;

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseReason {
    VoluntaryClose,
    TakeProfit,
    StopLoss,
    Liquidation,
    Adl,
}
