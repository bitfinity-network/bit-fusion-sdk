mod used_utxo_details;
mod utxo_details;
mod utxo_key;

use bitcoin::hashes::Hash;
use bitcoin::{Address, Txid};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{BTreeMapStructure, MemoryId, MemoryManager, StableBTreeMap};
use ord_rs::wallet::TxInputInfo;

use self::utxo_details::UtxoDetails;
pub use self::utxo_key::UtxoKey;
use crate::memory::{REVEAL_UTXOS_MEMORY_ID, USED_UTXOS_MEMORY_ID};

/// Data structure to keep track of utxos owned by the canister.
pub struct UtxoLedger<M: Memory> {
    /// contains a list of reveal utxos
    reveal_utxos: StableBTreeMap<UtxoKey, (), M>,
    /// contains a list of used utxos already processed in the deposit
    used_utxos: StableBTreeMap<UtxoKey, UtxoDetails, M>,
}

impl<M> UtxoLedger<M>
where
    M: Memory,
{
    pub fn new(memory_manager: &dyn MemoryManager<M, MemoryId>) -> Self {
        Self {
            reveal_utxos: StableBTreeMap::new(memory_manager.get(REVEAL_UTXOS_MEMORY_ID)),
            used_utxos: StableBTreeMap::new(memory_manager.get(USED_UTXOS_MEMORY_ID)),
        }
    }
}

/// Information about the unspent utxo.
#[derive(Debug, Clone)]
pub struct UnspentUtxoInfo {
    pub tx_input_info: TxInputInfo,
}

impl<M> UtxoLedger<M>
where
    M: Memory,
{
    /// Adds the reveal utxo to the store.
    pub fn deposit_reveal(&mut self, id: Txid, vout: u32) {
        self.reveal_utxos.insert(
            UtxoKey {
                tx_id: id.as_raw_hash().to_byte_array(),
                vout,
            },
            (),
        );

        log::debug!("Added utxo {id}:{vout} to reveal utxos",);
    }

    /// Marks the utxo as used.
    pub fn mark_as_used(&mut self, utxo: Utxo, address: &Address, derivation_path: Vec<Vec<u8>>) {
        let script = address.script_pubkey();

        let utxo_key = UtxoKey::from(&utxo.outpoint);

        self.used_utxos.insert(
            utxo_key,
            UtxoDetails {
                value: utxo.value,
                script_buf: script.clone().into_bytes(),
                derivation_path: derivation_path.clone(),
            },
        );

        log::debug!(
            "Marked utxo {}:{} with value {} as used",
            hex::encode(&utxo.outpoint.txid),
            utxo.outpoint.vout,
            utxo.value
        );
    }

    /// Checks if the used utxos contains the given utxo.
    pub fn used_utxo_contains(&self, utxo: &UtxoKey) -> bool {
        self.used_utxos.contains_key(utxo)
    }

    /// Checks if the unspent utxos contains the given utxo.
    pub fn reveal_utxos_contains(&self, utxo: &UtxoKey) -> bool {
        self.reveal_utxos.contains_key(utxo)
    }

    /// Removes the reveal utxo from the store.
    pub fn remove_reveal_utxo(&mut self, key: &UtxoKey) {
        self.reveal_utxos.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use ic_exports::ic_kit::MockContext;

    use super::*;
    use crate::canister::get_brc20_state;

    #[test]
    fn test_should_deposit_utxo() {
        MockContext::new().inject();

        let id = Txid::from_byte_array([0xde; 32]);

        let state = get_brc20_state();
        state.borrow_mut().ledger_mut().deposit_reveal(id, 1);

        // list unspent
        assert!(state.borrow().ledger().reveal_utxos_contains(&UtxoKey {
            tx_id: [0xde; 32],
            vout: 1
        }));
    }

    #[test]
    fn test_should_remove_reveal() {
        MockContext::new().inject();
        let id = Txid::from_byte_array([0xde; 32]);

        let state = get_brc20_state();
        state.borrow_mut().ledger_mut().deposit_reveal(id, 1);

        // list unspent
        assert!(state.borrow().ledger().reveal_utxos_contains(&UtxoKey {
            tx_id: [0xde; 32],
            vout: 1
        }));

        // list unspent
        assert!(state.borrow().ledger().reveal_utxos_contains(&UtxoKey {
            tx_id: [0xde; 32],
            vout: 1
        }));

        state
            .borrow_mut()
            .ledger_mut()
            .remove_reveal_utxo(&UtxoKey {
                tx_id: [0xde; 32],
                vout: 1,
            });

        assert!(!state.borrow().ledger().reveal_utxos_contains(&UtxoKey {
            tx_id: [0xde; 32],
            vout: 1
        }));
    }
}
