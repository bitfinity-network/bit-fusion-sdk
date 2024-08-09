use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::consensus::{Decodable, Encodable};
use bitcoin::hashes::Hash;
use bitcoin::{Address, Amount, Network, OutPoint, Transaction, TxOut, Txid};
use bridge_did::id256::Id256;
use bridge_utils::bft_events::BurntEventData;
use candid::types::{Serializer, Type};
use candid::{CandidType, Deserialize};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ic_exports::ic_kit::ic;
use ord_rs::wallet::{CreateEdictTxArgs, ScriptType, TxInputInfo};
use ord_rs::OrdTransactionBuilder;
use ordinals::RuneId;
use serde::{Deserializer, Serialize};

use crate::canister::get_rune_state;
use crate::core::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::interface::WithdrawError;
use crate::key::{get_derivation_path, get_derivation_path_ic, BtcSignerType};
use crate::rune_info::RuneInfo;
use crate::state::RuneState;

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct RuneWithdrawalPayload {
    pub rune_info: RuneInfo,
    pub amount: u128,
    pub request_ts: u64,
    pub sender: H160,
    pub dst_address: String,
}

impl RuneWithdrawalPayload {
    pub fn new(burnt_event_data: BurntEventData, state: &RuneState) -> Result<Self, WithdrawError> {
        let BurntEventData {
            recipient_id,
            amount,
            to_token,
            sender,
            ..
        } = burnt_event_data;

        let amount = amount.0.as_u128();

        let Ok(address_string) = String::from_utf8(recipient_id.clone()) else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode recipient address from raw data: {recipient_id:?}"
            )));
        };

        let Ok(address) = Address::from_str(&address_string) else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode recipient address from string: {address_string}"
            )));
        };

        let Some(token_id) = Id256::from_slice(&to_token) else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode token id from the value {to_token:?}"
            )));
        };

        let Ok(rune_id) = token_id.try_into() else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode rune id from the token id {to_token:?}"
            )));
        };

        let Some(rune_info) = state.rune_info(rune_id) else {
            // We don't need to request the list from the indexer at this point. This operation is
            // called only when some tokens are burned, which means they have been minted before,
            // and that means that we already received the rune info from the indexer.
            return Err(WithdrawError::InvalidRequest(format!(
                "Invalid rune id: {rune_id}. No such rune id in the rune list received from the indexer."
            )));
        };

        Ok(Self {
            rune_info,
            amount,
            request_ts: ic::time(),
            sender,
            dst_address: address.assume_checked().to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DidTransaction(Transaction);

impl CandidType for DidTransaction {
    fn _ty() -> Type {
        <Vec<u8> as CandidType>::_ty()
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;
        let mut bytes = vec![];
        self.0.consensus_encode(&mut bytes).map_err(Error::custom)?;

        bytes.idl_serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DidTransaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <Vec<u8> as Deserialize<'de>>::deserialize(deserializer)?;
        let tx =
            Transaction::consensus_decode(&mut &bytes[..]).map_err(serde::de::Error::custom)?;

        Ok(Self(tx))
    }
}

impl Serialize for DidTransaction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::Error;

        let mut bytes = vec![];
        self.0.consensus_encode(&mut bytes).map_err(Error::custom)?;
        serializer.serialize_bytes(&bytes)
    }
}

impl From<Transaction> for DidTransaction {
    fn from(value: Transaction) -> Self {
        Self(value)
    }
}

impl From<DidTransaction> for Transaction {
    fn from(value: DidTransaction) -> Self {
        value.0
    }
}

pub(crate) struct Withdrawal<UTXO: UtxoProvider> {
    state: Rc<RefCell<RuneState>>,
    utxo_provider: UTXO,
    signer: BtcSignerType,
    network: Network,
}

impl Withdrawal<IcUtxoProvider> {
    pub fn new(state: Rc<RefCell<RuneState>>) -> Self {
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

    pub fn get() -> Self {
        Self::new(get_rune_state())
    }
}

impl<UTXO: UtxoProvider> Withdrawal<UTXO> {
    pub async fn create_withdrawal_transaction(
        &self,
        payload: RuneWithdrawalPayload,
    ) -> Result<Transaction, WithdrawError> {
        let dst_address = payload.dst_address;

        let RuneWithdrawalPayload {
            rune_info,
            amount,
            sender,
            ..
        } = payload;

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

        let Ok(dst_address) = Address::from_str(&dst_address) else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode recipient address from string: {dst_address}"
            )));
        };

        let tx = self
            .build_withdraw_transaction(
                amount,
                dst_address.clone().assume_checked(),
                funding_address,
                rune_info.id(),
                utxos.clone(),
            )
            .await?;

        {
            let mut state = self.state.borrow_mut();
            let ledger = state.ledger_mut();
            for utxo in utxos {
                ledger.mark_as_used(utxo.outpoint.into(), dst_address.clone().assume_checked());
            }
        }

        Ok(tx)
    }

    pub async fn send_transaction(&self, tx: Transaction) -> Result<(), WithdrawError> {
        self.utxo_provider.send_tx(&tx).await?;

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

        Ok(())
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
