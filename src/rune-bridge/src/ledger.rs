use std::borrow::Cow;
use std::mem::size_of;

use bitcoin::hashes::sha256d::Hash;
use bitcoin::{Address, Amount, OutPoint, TxOut, Txid};
use candid::{CandidType, Decode, Encode};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ic_exports::ic_kit::ic;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{BTreeMapStructure, Bound, StableBTreeMap, Storable, VirtualMemory};
use ord_rs::wallet::TxInputInfo;
use serde::Deserialize;

use crate::key::{ic_dp_to_derivation_path, IcBtcSigner};
use crate::memory::{LEDGER_MEMORY_ID, MEMORY_MANAGER, USED_UTXOS_REGISTRY_MEMORY_ID};

/// Data structure to keep track of utxos owned by the canister.
pub struct UtxoLedger {
    utxo_storage: StableBTreeMap<UtxoKey, UtxoDetails, VirtualMemory<DefaultMemoryImpl>>,
    used_utxos_registry: StableBTreeMap<UtxoKey, UsedUtxoDetails, VirtualMemory<DefaultMemoryImpl>>,
}

impl Default for UtxoLedger {
    fn default() -> Self {
        Self {
            utxo_storage: StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(LEDGER_MEMORY_ID))),
            used_utxos_registry: StableBTreeMap::new(
                MEMORY_MANAGER.with(|mm| mm.get(USED_UTXOS_REGISTRY_MEMORY_ID)),
            ),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, CandidType, Deserialize)]
pub struct UtxoKey {
    tx_id: [u8; 32],
    vout: u32,
}

impl Storable for UtxoKey {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = Encode!(self).expect("cannot serialize utxo key");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("cannot deserialize utxo key")
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: 60,
        is_fixed_size: true,
    };
}

impl From<OutPoint> for UtxoKey {
    fn from(value: OutPoint) -> Self {
        Self {
            tx_id: *<Hash as AsRef<[u8; 32]>>::as_ref(&value.txid.to_raw_hash()),
            vout: value.vout,
        }
    }
}

impl From<&Outpoint> for UtxoKey {
    fn from(value: &Outpoint) -> Self {
        Self {
            tx_id: value.txid.clone().try_into().expect("invalid tx id"),
            vout: value.vout,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, CandidType, Deserialize)]
pub struct UtxoDetails {
    value: u64,
    script_buf: Vec<u8>,
    derivation_path: Vec<Vec<u8>>,
}

impl UtxoDetails {
    const MAX_SCRIPT_SIZE: u32 = 128;
    const DERIVATION_PATH_SIZE: u32 = IcBtcSigner::DERIVATION_PATH_SIZE;
}

impl Storable for UtxoDetails {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = Encode!(self).expect("failed to serialize utxo");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to deserialize utxo")
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: size_of::<u64>() as u32 + Self::MAX_SCRIPT_SIZE + Self::DERIVATION_PATH_SIZE,
        is_fixed_size: false,
    };
}

#[derive(Debug, Clone, Eq, PartialEq, CandidType, Deserialize)]
pub struct UsedUtxoDetails {
    pub used_at: u64,
}

impl Storable for UsedUtxoDetails {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = Encode!(self).expect("failed to serialize utxo");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to deserialize utxo")
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: size_of::<u64>() as u32,
        is_fixed_size: false,
    };
}

impl UtxoLedger {
    /// Adds the utxo to the store.
    pub fn deposit(&mut self, utxos: &[Utxo], address: &Address, derivation_path: Vec<Vec<u8>>) {
        let script = address.script_pubkey();
        for utxo in utxos {
            self.utxo_storage.insert(
                (&utxo.outpoint).into(),
                UtxoDetails {
                    value: utxo.value,
                    script_buf: script.clone().into_bytes(),
                    derivation_path: derivation_path.clone(),
                },
            );

            log::debug!(
                "Added utxo {}:{} with value {} to the ledger",
                hex::encode(&utxo.outpoint.txid),
                utxo.outpoint.vout,
                utxo.value
            );
        }
    }

    /// Lists all utxos in the store.
    pub fn load_all(&self) -> (Vec<UtxoKey>, Vec<TxInputInfo>) {
        self.utxo_storage
            .iter()
            .map(|(key, details)| {
                (
                    key,
                    TxInputInfo {
                        outpoint: OutPoint {
                            txid: Txid::from_raw_hash(*Hash::from_bytes_ref(&key.tx_id)),
                            vout: key.vout,
                        },
                        tx_out: TxOut {
                            value: Amount::from_sat(details.value),
                            script_pubkey: details.script_buf.into(),
                        },
                        derivation_path: ic_dp_to_derivation_path(&details.derivation_path)
                            .expect("invalid derivation path"),
                    },
                )
            })
            .unzip()
    }

    /// Marks the utxo as used.
    pub fn mark_as_used(&mut self, key: UtxoKey) {
        self.used_utxos_registry.insert(
            key,
            UsedUtxoDetails {
                used_at: ic::time(),
            },
        );
    }

    /// Lists all used utxos in the store.
    pub fn load_used_utxos(&self) -> Vec<(UtxoKey, UsedUtxoDetails, UtxoDetails)> {
        self.used_utxos_registry
            .iter()
            .filter_map(|(key, used_details)| {
                self.utxo_storage
                    .get(&key)
                    .map(|details| (key, used_details.clone(), details.clone()))
            })
            .collect()
    }

    /// Removes the utxo from the store.
    pub fn remove_utxo(&mut self, key: &UtxoKey) {
        self.utxo_storage.remove(key);
        self.used_utxos_registry.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bitcoin::{Network, PublicKey};
    use did::H160;

    use super::*;
    use crate::key::get_derivation_path_ic;

    #[test]
    fn key_serialization() {
        let key = UtxoKey {
            tx_id: [123; 32],
            vout: 544331,
        };

        let serialized = key.to_bytes();
        let Bound::Bounded { max_size, .. } = UtxoKey::BOUND else {
            panic!("Key is unbounded");
        };

        assert_eq!(serialized.len() as u32, max_size);
        let deserialized = UtxoKey::from_bytes(serialized);
        assert_eq!(deserialized, key);
    }

    #[test]
    fn value_serialization() {
        let address = Address::p2wpkh(
            &PublicKey::from_str(
                "038f47dcd43ba6d97fc9ed2e3bba09b175a45fac55f0683e8cf771e8ced4572354",
            )
            .unwrap(),
            Network::Signet,
        )
        .unwrap();
        let derivation_path = get_derivation_path_ic(
            &H160::from_hex_str("0x0dc9f6938e9b47fd8553df50bcbdb62d67239007").unwrap(),
        );
        let value = UtxoDetails {
            value: 100500,
            script_buf: address.script_pubkey().to_bytes(),
            derivation_path,
        };

        let serialized = value.to_bytes();
        let Bound::Bounded { max_size, .. } = UtxoDetails::BOUND else {
            panic!("Key is unbounded");
        };

        assert!((serialized.len() as u32) < max_size);
        let deserialized = UtxoDetails::from_bytes(serialized);
        assert_eq!(deserialized, value);
    }
}
