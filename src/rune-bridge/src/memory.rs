//! Memory IDs for the Bridge canister.
//!
//! DO NOT USE MEMORY IDS BELOW 100, as they are reserved for sdk use.

use ic_stable_structures::MemoryId;

pub const CONFIG_MEMORY_ID: MemoryId = MemoryId::new(100);
pub const DEPOSITED_UTXOS_MEMORY_ID: MemoryId = MemoryId::new(101);
pub const USED_UTXOS_MEMORY_ID: MemoryId = MemoryId::new(102);
pub const RUNE_INFO_BY_UTXO_MEMORY_ID: MemoryId = MemoryId::new(103);
pub const MASTER_KEY_MEMORY_ID: MemoryId = MemoryId::new(104);
