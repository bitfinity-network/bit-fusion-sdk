//! Memory ids for bridge
//!
//! DO NOT USE ANY MEMORY ID BELOW 10, since used by the sdk

use ic_stable_structures::MemoryId;

pub const CONFIG_MEMORY_ID: MemoryId = MemoryId::new(10);
pub const MASTER_KEY_MEMORY_ID: MemoryId = MemoryId::new(12);
pub const UNUSED_UTXOS_MEMORY_ID: MemoryId = MemoryId::new(13);
pub const USED_UTXOS_MEMORY_ID: MemoryId = MemoryId::new(14);
