mod brc20;
mod bridge_data;
mod rune;

use std::time::Duration;

pub use brc20::*;
pub use bridge_data::*;
pub use rune::*;

pub const DEFAULT_DEPOSIT_FEE: u64 = 100_000;
pub const DEFAULT_MEMPOOL_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

/// Minimum number of indexers required to start the bridge.
pub const MIN_INDEXERS: usize = 2;
pub const DEFAULT_INDEXER_CONSENSUS_THRESHOLD: u8 = 2;