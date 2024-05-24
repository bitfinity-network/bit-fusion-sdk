use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use ::bitcoin::Address;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{
    self, BitcoinNetwork, GetUtxosRequest, Utxo, UtxoFilter,
};
use ic_exports::ic_kit::RejectionCode;

use crate::ledger::UtxoKey;
use crate::state::State;

const AVG_BLOCK_TIME: Duration = Duration::from_secs(60 * 10); // 10 minutes

/// Task to remove used UTXOs from the ledger.
/// It also remark as available to be spent those utxos which haven't been spent for some reason.
pub struct RemoveUsedUtxosTask {
    state: Rc<RefCell<State>>,
}

impl From<Rc<RefCell<State>>> for RemoveUsedUtxosTask {
    fn from(state: Rc<RefCell<State>>) -> Self {
        Self { state }
    }
}

impl RemoveUsedUtxosTask {
    /// Run the task.
    pub async fn run(self) {
        let time_now = Duration::from_nanos(ic_exports::ic_cdk::api::time());
        let min_confirmations = self.state.borrow().min_confirmations();
        let minimum_confirmation_time = min_confirmations * AVG_BLOCK_TIME;

        let utxos_to_check = self
            .state
            .borrow()
            .ledger()
            .load_used_utxos()
            .into_iter()
            .filter(|(_, details)| {
                Duration::from_nanos(details.used_at) + minimum_confirmation_time <= time_now
            })
            .collect::<Vec<_>>();

        let network = self.state.borrow().network();
        let mut utxos_by_owner = HashMap::new();
        for (key, used_utxo_info) in utxos_to_check {
            // try to get the previous owner address of the utxo; otherwise remove it
            let owner_address = match used_utxo_info.owner_address(network) {
                Ok(address) => address,
                Err(err) => {
                    log::error!("invalid owner address: {}; removing used utxo", err);
                    self.state.borrow_mut().ledger_mut().remove_spent_utxo(&key);
                    continue;
                }
            };
            // insert the utxo into the owner's list
            utxos_by_owner
                .entry(owner_address)
                .or_insert_with(Vec::new)
                .push(key);
        }

        // iter owners
        let btc_network = self.state.borrow().ic_btc_network();
        for (owner, utxos) in utxos_by_owner {
            let owner_utxos =
                match Self::get_owner_utxos(&owner, btc_network, min_confirmations).await {
                    Ok(utxos) => utxos,
                    Err((code, msg)) => {
                        log::error!("failed to get owner {owner} utxos: {msg} ({code:?})");
                        continue;
                    }
                };
            // get spent utxos
            utxos
                .into_iter()
                .for_each(|key| self.remove_used_utxo(&key, &owner_utxos));
        }
    }

    /// Check whether the provided UTXO is spent or not.
    ///
    /// If the UTXO is spent, it will be removed from the ledger.
    /// Otherwise, the UTXO will be removed only from the used UTXOs registry.
    fn remove_used_utxo(&self, used_utxo_key: &UtxoKey, owner_utxos: &[Utxo]) {
        let is_utxo_spent = !owner_utxos.iter().any(|owner_utxo| {
            owner_utxo.outpoint.txid == used_utxo_key.tx_id
                && owner_utxo.outpoint.vout == used_utxo_key.vout
        });

        if is_utxo_spent {
            self.state
                .borrow_mut()
                .ledger_mut()
                .remove_spent_utxo(used_utxo_key);
            log::info!("removed spent utxo: {used_utxo_key}");
        } else {
            // mark as unspent
            self.state
                .borrow_mut()
                .ledger_mut()
                .remove_unspent_utxo(used_utxo_key);
            log::info!("marked unspent utxo: {used_utxo_key}");
        }
    }

    /// Get all UTXOs owned by the given owner.
    async fn get_owner_utxos(
        owner: &Address,
        btc_network: BitcoinNetwork,
        min_confirmations: u32,
    ) -> Result<Vec<Utxo>, (RejectionCode, String)> {
        log::debug!("getting utxos for owner {owner}");
        let mut filter = UtxoFilter::MinConfirmations(min_confirmations);
        let mut utxos = vec![];
        loop {
            let response = bitcoin::bitcoin_get_utxos(GetUtxosRequest {
                address: owner.to_string(),
                network: btc_network,
                filter: Some(filter),
            })
            .await
            .map(|(value,)| value)?;

            utxos.extend(response.utxos);
            match response.next_page {
                None => break,
                Some(page) => {
                    filter = UtxoFilter::Page(page);
                }
            }
        }

        log::debug!("got {} utxos for owner {owner}", utxos.len());
        Ok(utxos)
    }
}
