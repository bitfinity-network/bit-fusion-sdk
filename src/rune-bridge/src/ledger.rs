use std::borrow::Cow;
use std::cmp::Ordering;

use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{
    BTreeMapStructure, Bound, CellStructure, StableBTreeMap, StableCell, Storable, VirtualMemory,
};

use crate::memory::{BALANCE_MEMORY_ID, LEDGER_MEMORY_ID, MEMORY_MANAGER};

pub struct Ledger {
    utxo_storage: StableBTreeMap<StorableOutpoint, StorableUtxo, VirtualMemory<DefaultMemoryImpl>>,
    balance_storage: StableCell<u128, VirtualMemory<DefaultMemoryImpl>>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            utxo_storage: StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(LEDGER_MEMORY_ID))),
            balance_storage: StableCell::new(
                MEMORY_MANAGER.with(|mm| mm.get(BALANCE_MEMORY_ID)),
                0u128,
            )
            .expect("Failed to create stable storage for balances"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct StorableOutpoint(Outpoint);

impl StorableOutpoint {
    const SIZE: u32 = 32 + 4;
}

impl Storable for StorableOutpoint {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut bytes = self.0.txid.clone();
        bytes.append(&mut self.0.vout.to_be_bytes().to_vec());

        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let txid = bytes[0..32].to_vec();
        let vout = u32::from_be_bytes(bytes[32..36].try_into().expect("invalid bytes length"));

        Self(Outpoint { txid, vout })
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: Self::SIZE,
        is_fixed_size: true,
    };
}

impl PartialOrd<Self> for StorableOutpoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Outpoint> for StorableOutpoint {
    fn from(value: Outpoint) -> Self {
        Self(value)
    }
}

impl Ord for StorableOutpoint {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .txid
            .cmp(&other.0.txid)
            .then(self.0.vout.cmp(&other.0.vout))
    }
}

struct StorableUtxo(Utxo);

impl Storable for StorableUtxo {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut bytes = self.0.outpoint.txid.clone();
        bytes.append(&mut self.0.outpoint.vout.to_be_bytes().to_vec());
        bytes.append(&mut self.0.height.to_be_bytes().to_vec());
        bytes.append(&mut self.0.value.to_be_bytes().to_vec());

        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let txid = bytes[0..32].to_vec();
        let vout = u32::from_be_bytes(bytes[32..36].try_into().expect("invalid bytes length"));
        let outpoint = Outpoint { txid, vout };
        let height = u32::from_be_bytes(bytes[36..40].try_into().expect("invalid bytes length"));
        let value = u64::from_be_bytes(bytes[40..44].try_into().expect("invalid bytes length"));

        Self(Utxo {
            outpoint,
            height,
            value,
        })
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: StorableOutpoint::SIZE + 4 + 8,
        is_fixed_size: true,
    };
}

impl From<Utxo> for StorableUtxo {
    fn from(value: Utxo) -> Self {
        Self(value)
    }
}

impl Ledger {
    pub fn deposit(&mut self, utxos: &[Utxo], rune_amount: u128) {
        for utxo in utxos {
            self.utxo_storage
                .insert(utxo.outpoint.clone().into(), utxo.clone().into());
        }

        let curr_balance = *self.balance_storage.get();
        self.balance_storage
            .set(curr_balance + rune_amount)
            .expect("Failed to store balance");
    }
}
