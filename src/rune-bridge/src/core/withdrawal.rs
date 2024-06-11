use crate::core::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::interface::WithdrawError;
use crate::key::{get_derivation_path, get_derivation_path_ic, BtcSignerType};
use crate::state::State;
use bitcoin::hashes::Hash;
use bitcoin::{Address, Amount, Network, OutPoint, Transaction, TxOut, Txid};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ord_rs::wallet::{CreateEdictTxArgs, ScriptType, TxInputInfo};
use ord_rs::OrdTransactionBuilder;
use ordinals::RuneId;
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) struct Withdrawal<UTXO: UtxoProvider> {
    state: Rc<RefCell<State>>,
    utxo_provider: UTXO,
    signer: BtcSignerType,
    network: Network,
}

impl Withdrawal<IcUtxoProvider> {
    pub fn new(state: Rc<RefCell<State>>) -> Self {
        let state_ref = state.borrow();

        let network = state_ref.network();
        let ic_network = state_ref.ic_btc_network();
        let signer = state_ref.btc_signer();

        drop(state_ref);

        Self {
            state,
            network,
            signer,
            utxo_provider: IcUtxoProvider::new(ic_network),
        }
    }
}

impl<UTXO: UtxoProvider> Withdrawal<UTXO> {
    pub async fn withdraw(
        &self,
        amount: u128,
        rune_id: RuneId,
        sender: H160,
        dst_address: Address,
    ) -> Result<Txid, WithdrawError> {
        let (_, mut utxos) = self.state.borrow().ledger().load_unspent_utxos();
        let funding_address = self.get_transit_address(&sender).await;
        let mut funding_utxos: Vec<_> = self
            .utxo_provider
            .get_utxos(&funding_address)
            .await
            .map_err(|_e| WithdrawError::NoInputs)?
            .utxos
            .into_iter()
            .map(|utxo| TxInputInfo {
                outpoint: OutPoint {
                    txid: Txid::from_slice(&utxo.outpoint.txid).unwrap(),
                    vout: utxo.outpoint.vout,
                },
                tx_out: TxOut {
                    value: Amount::from_sat(utxo.value),
                    script_pubkey: funding_address.script_pubkey(),
                },
                derivation_path: get_derivation_path(&sender),
            })
            .collect();

        utxos.append(&mut funding_utxos);

        let tx = self
            .build_withdraw_transaction(
                amount,
                dst_address.clone(),
                funding_address,
                rune_id,
                utxos.clone(),
            )
            .await?;
        self.utxo_provider.send_tx(&tx).await?;

        {
            let mut state = self.state.borrow_mut();
            let ledger = state.ledger_mut();
            for utxo in utxos {
                ledger.mark_as_used(utxo.outpoint.into(), dst_address.clone());
            }
        }

        let change_address = self.get_change_address().await;

        const CHANGE_OUTPOINT_INDEX: usize = 1;
        // Make sure that the transaction builder code is not change and the change outpoint
        // is where we expect it to be. If not, panic until the code of the canister is fixed.
        assert_eq!(
            tx.output[CHANGE_OUTPOINT_INDEX].script_pubkey,
            change_address.script_pubkey()
        );

        let change_utxo = Utxo {
            outpoint: Outpoint {
                txid: tx.txid().as_byte_array().to_vec(),
                vout: CHANGE_OUTPOINT_INDEX as u32,
            },
            value: tx.output[CHANGE_OUTPOINT_INDEX].value.to_sat(),
            height: 0,
        };

        self.state.borrow_mut().ledger_mut().deposit(
            &[change_utxo],
            &change_address,
            self.get_change_derivation_path(),
        );

        Ok(tx.txid())
    }

    async fn get_transit_address(&self, eth_address: &H160) -> Address {
        self.signer
            .get_transit_address(eth_address, self.network)
            .await
    }

    pub async fn build_withdraw_transaction(
        &self,
        amount: u128,
        dst_address: Address,
        change_address: Address,
        rune: RuneId,
        inputs: Vec<TxInputInfo>,
    ) -> Result<Transaction, WithdrawError> {
        if inputs.is_empty() {
            return Err(WithdrawError::NoInputs);
        }

        let public_key = self.state.borrow().public_key();
        let wallet = self.state.borrow().wallet();

        let builder = OrdTransactionBuilder::new(public_key, ScriptType::P2WSH, wallet);

        let rune_change_address = self.get_change_address().await;
        let fee_rate = self.utxo_provider.get_fee_rate().await?;

        let args = CreateEdictTxArgs {
            rune,
            inputs,
            destination: dst_address,
            change_address,
            rune_change_address,
            amount,
            fee_rate,
        };
        let unsigned_tx = builder.create_edict_transaction(&args).map_err(|err| {
            log::warn!("Failed to create withdraw transaction: {err:?}");
            WithdrawError::TransactionCreation
        })?;
        let signed_tx = builder
            .sign_transaction(&unsigned_tx, &args.inputs)
            .await
            .map_err(|err| {
                log::error!("Failed to sign withdraw transaction: {err:?}");
                WithdrawError::TransactionSigning
            })?;

        Ok(signed_tx)
    }

    async fn get_change_address(&self) -> Address {
        self.get_transit_address(&H160::default()).await
    }

    fn get_change_derivation_path(&self) -> Vec<Vec<u8>> {
        get_derivation_path_ic(&H160::default())
    }
}
