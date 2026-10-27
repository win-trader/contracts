use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CexFeedError {
    Unauthorized = 600,
    NotPublisher = 601,

    InvalidConfig = 610,
    InvalidPrice = 611,

    NoPendingSources = 620,
    SourcesTimelockNotElapsed = 621,
    SourcesProposalExpired = 622,
}
