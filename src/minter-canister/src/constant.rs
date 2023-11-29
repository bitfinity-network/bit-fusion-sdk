use ic_stable_structures::MemoryId;

pub const CONFIG_MEMORY_ID: MemoryId = MemoryId::new(80);
pub const NONCE_MEMORY_ID: MemoryId = MemoryId::new(81);
pub const TX_SIGNER_MEMORY_ID: MemoryId = MemoryId::new(83);
pub const MINT_ORDERS_MEMORY_ID: MemoryId = MemoryId::new(85);
pub const USER_OPERATION_POINTS_MEMORY_ID: MemoryId = MemoryId::new(86);
pub const OPERATION_PRICING_MEMORY_ID: MemoryId = MemoryId::new(87);
pub const NONCES_COUNTER_MEMORY_ID: MemoryId = MemoryId::new(88);
pub const LOG_SETTINGS_MEMORY_ID: MemoryId = MemoryId::new(89);

pub const DEFAULT_TX_GAS_LIMIT: u64 = 3_000_000;
pub const DEFAULT_CHAIN_ID: u32 = 355113;
pub const DEFAULT_GAS_PRICE: u64 = 10_000;

pub const IC_CHAIN_ID: u32 = 0;
