use std::sync::atomic::AtomicU32;

pub(crate) const MAINNET_CHAIN_ID: u32 = 0;
pub(crate) const TESTNET_CHAIN_ID: u32 = 1;
pub(crate) const REGTEST_CHAIN_ID: u32 = 2;
pub(crate) const EVM_INFO_INITIALIZATION_RETRIES: u32 = 5;
pub(crate) const EVM_INFO_INITIALIZATION_RETRY_DELAY_SEC: u32 = 2;
pub(crate) const EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER: u32 = 2;
pub(crate) static NONCE: AtomicU32 = AtomicU32::new(0);
pub(crate) const BRC20_TICKER_LEN: usize = 4;
pub(crate) const HTTP_OUTCALL_PER_CALL_COST: u128 = 171_360_000;
pub(crate) const HTTP_OUTCALL_REQ_PER_BYTE_COST: u128 = 13_600;
pub(crate) const HTTP_OUTCALL_RES_PER_BYTE_COST: u128 = 27_200;
pub(crate) const HTTP_OUTCALL_RES_DEFAULT_SIZE: u64 = 2097152;
