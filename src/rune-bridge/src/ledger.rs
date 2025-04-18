mod used_utxo_details;
mod utxo_details;
mod utxo_key;
mod utxo_runes;

use std::collections::HashMap;

use bitcoin::hashes::sha256d::Hash;
use bitcoin::{Address, Amount, OutPoint, TxOut, Txid};
use bridge_did::runes::RuneInfo;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_exports::ic_kit::ic;
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{BTreeMapStructure, MemoryId, MemoryManager, StableBTreeMap};
use ord_rs::wallet::TxInputInfo;

use self::used_utxo_details::UsedUtxoDetails;
use self::utxo_details::UtxoDetails;
pub use self::utxo_key::UtxoKey;
use self::utxo_runes::UtxoRunes;
use crate::key::{KeyError, ic_dp_to_derivation_path};
use crate::memory::{DEPOSITED_UTXOS_MEMORY_ID, RUNE_INFO_BY_UTXO_MEMORY_ID, USED_UTXOS_MEMORY_ID};

/// Information about the unspent utxo.
#[derive(Debug, Clone)]
pub struct UnspentUtxoInfo {
    pub tx_input_info: TxInputInfo,
    pub rune_info: Vec<RuneInfo>,
}

/// Data structure to keep track of utxos owned by the canister.
pub struct UtxoLedger<M: Memory> {
    rune_info_by_utxo: StableBTreeMap<UtxoKey, UtxoRunes, M>,
    /// contains a list of utxos on the main canister account (which get there as a change from withdrawal transactions)
    deposited_utxos: StableBTreeMap<UtxoKey, UtxoDetails, M>,
    /// contains a list of utxos that are on user's deposit address.
    used_utxos: StableBTreeMap<UtxoKey, UsedUtxoDetails, M>,
}

impl<M> UtxoLedger<M>
where
    M: Memory,
{
    pub fn new(memory_manager: &dyn MemoryManager<M, MemoryId>) -> Self {
        Self {
            rune_info_by_utxo: StableBTreeMap::new(memory_manager.get(RUNE_INFO_BY_UTXO_MEMORY_ID)),
            deposited_utxos: StableBTreeMap::new(memory_manager.get(DEPOSITED_UTXOS_MEMORY_ID)),
            used_utxos: StableBTreeMap::new(memory_manager.get(USED_UTXOS_MEMORY_ID)),
        }
    }

    /// Adds the utxo to the store.
    pub fn deposit(
        &mut self,
        utxo: Utxo,
        address: &Address,
        derivation_path: Vec<Vec<u8>>,
        rune_info: Vec<RuneInfo>,
    ) {
        log::debug!("Depositing utxo: {:?}", utxo);
        log::debug!("Depositing utxo with address: {:?}", address);
        log::debug!(
            "Depositing utxo with derivation path: {:?}",
            derivation_path
        );
        log::debug!("Depositing utxo with rune info: {:?}", rune_info);
        let script = address.script_pubkey();

        let utxo_key = UtxoKey::from(&utxo.outpoint);

        self.deposited_utxos.insert(
            utxo_key,
            UtxoDetails {
                value: utxo.value,
                script_buf: script.clone().into_bytes(),
                derivation_path: derivation_path.clone(),
            },
        );

        // Add rune info if it is present
        if !rune_info.is_empty() {
            self.rune_info_by_utxo.insert(utxo_key, rune_info.into());
        }

        log::debug!(
            "Added utxo {}:{} with value {} to the ledger",
            hex::encode(&utxo.outpoint.txid),
            utxo.outpoint.vout,
            utxo.value
        );
    }

    /// Lists all unspent utxos in the store.
    pub fn load_unspent_utxos(&self) -> Result<HashMap<UtxoKey, UnspentUtxoInfo>, KeyError> {
        let mut map = HashMap::new();

        for (key, details) in self.deposited_utxos.iter() {
            map.insert(
                key,
                UnspentUtxoInfo {
                    tx_input_info: TxInputInfo {
                        outpoint: OutPoint {
                            txid: Txid::from_raw_hash(*Hash::from_bytes_ref(&key.tx_id)),
                            vout: key.vout,
                        },
                        tx_out: TxOut {
                            value: Amount::from_sat(details.value),
                            script_pubkey: details.script_buf.into(),
                        },
                        derivation_path: ic_dp_to_derivation_path(&details.derivation_path)?,
                    },
                    rune_info: self
                        .rune_info_by_utxo
                        .get(&key)
                        .map(|rune_info| rune_info.runes().to_vec())
                        .unwrap_or_default(),
                },
            );
        }

        Ok(map)
    }

    /// Marks the utxo as used.
    pub fn mark_as_used(&mut self, key: UtxoKey, address: Address) {
        self.used_utxos.insert(
            key,
            UsedUtxoDetails {
                used_at: ic::time(),
                owner_address: address.to_string(),
            },
        );
        self.deposited_utxos.remove(&key);

        log::trace!("Utxo {key} is marked as used.");
    }

    /// Lists all used utxos in the store.
    pub fn load_used_utxos(&self) -> Vec<(UtxoKey, UsedUtxoDetails)> {
        self.used_utxos.iter().collect()
    }

    /// Removes the spent utxo from the store.
    ///
    /// It gets removed from both the utxo storage, the rune info registry and the used utxos registry.
    pub fn remove_spent_utxo(&mut self, key: &UtxoKey) {
        self.deposited_utxos.remove(key);
        self.used_utxos.remove(key);
        self.rune_info_by_utxo.remove(key);
    }

    /// Removes the unspent utxo from the store.
    /// It gets removed only from the `used_utxos_registry`
    pub fn remove_unspent_utxo(&mut self, key: &UtxoKey) {
        self.used_utxos.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bridge_did::runes::RuneName;
    use ic_exports::ic_cdk::api::management_canister::bitcoin::Outpoint;
    use ic_exports::ic_kit::MockContext;
    use ordinals::Rune;

    use super::*;
    use crate::canister::get_rune_state;

    #[test]
    fn test_should_deposit_utxo() {
        MockContext::new().inject();
        let address = Address::from_str("bc1quyjp8qxkdc22cej962xaydd5arm7trwtcnkzks")
            .unwrap()
            .assume_checked();

        let utxo = Utxo {
            outpoint: Outpoint {
                txid: vec![0xde; 32],
                vout: 1,
            },
            value: 0,
            height: 0,
        };

        let state = get_rune_state();
        state
            .borrow_mut()
            .ledger_mut()
            .deposit(utxo, &address, vec![], vec![]);

        // list unspent
        let keys = state
            .borrow()
            .ledger()
            .load_unspent_utxos()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].tx_id, [0xde; 32]);
        assert_eq!(keys[0].vout, 1);
    }

    #[test]
    fn test_should_mark_used_utxo() {
        MockContext::new().inject();
        let address = Address::from_str("bc1quyjp8qxkdc22cej962xaydd5arm7trwtcnkzks")
            .unwrap()
            .assume_checked();

        let utxo = Utxo {
            outpoint: Outpoint {
                txid: vec![0xde; 32],
                vout: 1,
            },
            value: 0,
            height: 0,
        };

        let state = get_rune_state();
        state
            .borrow_mut()
            .ledger_mut()
            .deposit(utxo, &address, vec![], vec![]);

        let keys = state
            .borrow()
            .ledger()
            .load_unspent_utxos()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        state
            .borrow_mut()
            .ledger_mut()
            .mark_as_used(keys[0], address.clone());

        let used_utxos = state.borrow().ledger().load_used_utxos();
        assert_eq!(used_utxos.len(), 1);
        assert_eq!(used_utxos[0].0, keys[0]);
        assert_eq!(used_utxos[0].1.owner_address, address.to_string());
    }

    #[test]
    fn test_should_not_list_unspent_utxo_if_used() {
        MockContext::new().inject();
        let address = Address::from_str("bc1quyjp8qxkdc22cej962xaydd5arm7trwtcnkzks")
            .unwrap()
            .assume_checked();

        let utxos = [
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0xaa; 32],
                    vout: 1,
                },
                value: 0,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0xab; 32],
                    vout: 1,
                },
                value: 0,
                height: 0,
            },
        ];

        let state = get_rune_state();
        state
            .borrow_mut()
            .ledger_mut()
            .deposit(utxos[0].clone(), &address, vec![], vec![]);
        state
            .borrow_mut()
            .ledger_mut()
            .deposit(utxos[1].clone(), &address, vec![], vec![]);

        // mark first as spent
        state
            .borrow_mut()
            .ledger_mut()
            .mark_as_used(UtxoKey::from(&utxos[0].outpoint), address.clone());

        // load unspent
        let keys = state
            .borrow()
            .ledger()
            .load_unspent_utxos()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].tx_id.to_vec(), utxos[1].outpoint.txid);
        assert_eq!(keys[0].vout, utxos[1].outpoint.vout);

        // load used
        let used_utxos = state.borrow().ledger().load_used_utxos();
        assert_eq!(used_utxos.len(), 1);
        assert_eq!(used_utxos[0].0.tx_id.to_vec(), utxos[0].outpoint.txid);
        assert_eq!(used_utxos[0].0.vout, utxos[0].outpoint.vout);
    }

    #[test]
    fn test_should_remove_spent_utxo() {
        MockContext::new().inject();
        let address = Address::from_str("bc1quyjp8qxkdc22cej962xaydd5arm7trwtcnkzks")
            .unwrap()
            .assume_checked();

        let utxos = [
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0xaa; 32],
                    vout: 1,
                },
                value: 0,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0xab; 32],
                    vout: 1,
                },
                value: 0,
                height: 0,
            },
        ];

        let state = get_rune_state();
        state
            .borrow_mut()
            .ledger_mut()
            .deposit(utxos[0].clone(), &address, vec![], vec![]);
        state
            .borrow_mut()
            .ledger_mut()
            .deposit(utxos[1].clone(), &address, vec![], vec![]);

        // mark first as spent
        state
            .borrow_mut()
            .ledger_mut()
            .mark_as_used(UtxoKey::from(&utxos[0].outpoint), address.clone());

        // remove spent
        state
            .borrow_mut()
            .ledger_mut()
            .remove_spent_utxo(&UtxoKey::from(&utxos[0].outpoint));

        // check
        let keys = state
            .borrow()
            .ledger()
            .load_unspent_utxos()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].tx_id.to_vec(), utxos[1].outpoint.txid);
        assert_eq!(keys[0].vout, utxos[1].outpoint.vout);

        let used_utxos = state.borrow().ledger().load_used_utxos();
        assert_eq!(used_utxos.len(), 0);
    }

    #[test]
    fn test_should_remove_unspent_utxo() {
        MockContext::new().inject();
        let address = Address::from_str("bc1quyjp8qxkdc22cej962xaydd5arm7trwtcnkzks")
            .unwrap()
            .assume_checked();

        let utxos = [
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0xaa; 32],
                    vout: 1,
                },
                value: 0,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0xab; 32],
                    vout: 1,
                },
                value: 0,
                height: 0,
            },
        ];

        let state = get_rune_state();
        state
            .borrow_mut()
            .ledger_mut()
            .deposit(utxos[0].clone(), &address, vec![], vec![]);
        state
            .borrow_mut()
            .ledger_mut()
            .deposit(utxos[1].clone(), &address, vec![], vec![]);

        // mark first as spent
        state
            .borrow_mut()
            .ledger_mut()
            .mark_as_used(UtxoKey::from(&utxos[0].outpoint), address.clone());

        // remove spent
        state
            .borrow_mut()
            .ledger_mut()
            .remove_unspent_utxo(&UtxoKey::from(&utxos[0].outpoint));

        // check
        let mut keys = state
            .borrow()
            .ledger()
            .load_unspent_utxos()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();
        assert_eq!(keys.len(), 1);

        assert_eq!(keys[0].tx_id.to_vec(), utxos[1].outpoint.txid);
        assert_eq!(keys[0].vout, utxos[1].outpoint.vout);

        let used_utxos = state.borrow().ledger().load_used_utxos();
        assert_eq!(used_utxos.len(), 0);
    }

    #[test]
    fn test_should_deposit_utxo_with_rune_info() {
        MockContext::new().inject();
        let address = Address::from_str("bc1quyjp8qxkdc22cej962xaydd5arm7trwtcnkzks")
            .unwrap()
            .assume_checked();

        let utxo = Utxo {
            outpoint: Outpoint {
                txid: vec![0xde; 32],
                vout: 1,
            },
            value: 0,
            height: 0,
        };

        let rune_info = vec![
            RuneInfo {
                name: RuneName::from(Rune(0xdeadbeef)),
                decimals: 18,
                block: 0x1234567890abcdef,
                tx: 0x12345678,
            },
            RuneInfo {
                name: RuneName::from(Rune(0xcafebabe)),
                decimals: 18,
                block: 0x1234567890abcdef,
                tx: 0x12345678,
            },
        ];

        let key = UtxoKey::from(&utxo.outpoint);

        let state = get_rune_state();
        state
            .borrow_mut()
            .ledger_mut()
            .deposit(utxo, &address, vec![], rune_info.clone());

        // list unspent
        let utxos = state.borrow().ledger().load_unspent_utxos().unwrap();
        assert_eq!(utxos.len(), 1);
        assert!(utxos.contains_key(&key));

        assert_eq!(utxos.get(&key).unwrap().rune_info, rune_info);
    }
}
