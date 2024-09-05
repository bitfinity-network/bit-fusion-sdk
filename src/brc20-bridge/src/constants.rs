use std::time::Duration;

/// The interval at which the fee rate is updated (10 minutes)
pub const FEE_RATE_UPDATE_INTERVAL: Duration = Duration::from_secs(60 * 10);
/// Maximum amount of taproot keypair generation attempts before failing
pub const MAX_TAPROOT_KEYPAIR_GEN_ATTEMPTS: u64 = 250;
