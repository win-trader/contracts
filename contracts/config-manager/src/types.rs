pub use shared::{BorrowRateConfig, FeeConfig, FeeSplits, ProtocolLimits};

/// Role identifiers — canonical strings are defined in `shared::constants`.
pub mod roles {
    pub use shared::constants::{
        ROLE_ADMIN as DEFAULT_ADMIN,
        ROLE_UPGRADER as UPGRADER,
        ROLE_PAUSER as PAUSER,
        ROLE_KEEPER as KEEPER,
        ROLE_ORACLE as ORACLE,
    };
}
