use ic_stable_structures::MemoryId;

pub const CONFIG_MEMORY_ID: MemoryId = MemoryId::new(80);
pub const TX_SIGNER_MEMORY_ID: MemoryId = MemoryId::new(82);
pub const MINT_ORDERS_MEMORY_ID: MemoryId = MemoryId::new(83);
pub const NONCES_COUNTER_MEMORY_ID: MemoryId = MemoryId::new(84);
pub const LOG_SETTINGS_MEMORY_ID: MemoryId = MemoryId::new(85);
pub const PENDING_TASKS_MEMORY_ID: MemoryId = MemoryId::new(86);

pub const DEFAULT_TX_GAS_LIMIT: u64 = 3_000_000;

pub const IC_CHAIN_ID: u32 = 0;

pub const TASK_RETRY_DELAY_SECS: u32 = 5;
