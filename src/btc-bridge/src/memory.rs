//! Memory IDs for the BTC Bridge program.
//!
//! DO NOT USE MEMORY IDS BELOW 10, as they are reserved for sdk use.

use ic_stable_structures::MemoryId;

pub const BTC_CONFIG_MEMORY_ID: MemoryId = MemoryId::new(10);
pub const WRAPPED_TOKEN_CONFIG_MEMORY_ID: MemoryId = MemoryId::new(11);
