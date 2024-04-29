use std::sync::atomic::AtomicU32;

pub const MAINNET_CHAIN_ID: u32 = 0;
pub const TESTNET_CHAIN_ID: u32 = 1;
pub const REGTEST_CHAIN_ID: u32 = 2;
pub const EVM_INFO_INITIALIZATION_RETRIES: u32 = 5;
pub const EVM_INFO_INITIALIZATION_RETRY_DELAY_SEC: u32 = 2;
pub const EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER: u32 = 2;
pub const MAX_HTTP_RESPONSE_BYTES: u64 = 10_000;
pub const CYCLES_PER_HTTP_REQUEST: u128 = 100_000_000;
pub static NONCE: AtomicU32 = AtomicU32::new(0);
