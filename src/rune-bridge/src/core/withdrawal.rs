use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::consensus::{Decodable, Encodable};
use bitcoin::hashes::Hash;
use bitcoin::{Address, Amount, Network, OutPoint, Transaction, TxOut, Txid};
use bridge_utils::bft_bridge_api::BurntEventData;
use bridge_utils::operation_store::MinterOperationId;
use candid::types::{Serializer, Type};
use candid::{CandidType, Deserialize};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ic_exports::ic_kit::ic;
use minter_did::id256::Id256;
use ord_rs::wallet::{CreateEdictTxArgs, ScriptType, TxInputInfo};
use ord_rs::OrdTransactionBuilder;
use ordinals::RuneId;
use serde::Deserializer;

use crate::canister::get_operations_store;
use crate::core::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::interface::WithdrawError;
use crate::key::{get_derivation_path, get_derivation_path_ic, BtcSignerType};
use crate::operation::{OperationState, RuneOperationStore};
use crate::rune_info::RuneInfo;
use crate::state::State;

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct RuneWithdrawalPayload {
    rune_info: RuneInfo,
    amount: u128,
    request_ts: u64,
    sender: H160,
    dst_address: String,
    status: WithdrawalStatus,
}

impl RuneWithdrawalPayload {
    pub fn new(burnt_event_data: BurntEventData, state: &State) -> Self {
        let BurntEventData {
            recipient_id,
            amount,
            to_token,
            sender,
            ..
        } = burnt_event_data;

        let amount = amount.0.as_u128();

        let Ok(address_string) = String::from_utf8(recipient_id.clone()) else {
            return Self::invalid(format!(
                "Failed to decode recipient address from raw data: {recipient_id:?}"
            ));
        };

        let Ok(address) = Address::from_str(&address_string) else {
            return Self::invalid(format!(
                "Failed to decode recipient address from string: {address_string}"
            ));
        };

        let Some(token_id) = Id256::from_slice(&to_token) else {
            return Self::invalid(format!(
                "Failed to decode token id from the value {to_token:?}"
            ));
        };

        let Ok(rune_id) = token_id.try_into() else {
            return Self::invalid(format!(
                "Failed to decode rune id from the token id {to_token:?}"
            ));
        };

        let Some(rune_info) = state.rune_info(rune_id) else {
            // We don't need to request the list from the indexer at this point. This operation is
            // called only when some tokens are burned, which means they have been minted before,
            // and that means that we already received the rune info from the indexer.
            return Self::invalid(format!(
                "Invalid rune id: {rune_id}. No such rune id in the rune list received from the indexer."
            ));
        };

        Self {
            rune_info,
            amount,
            request_ts: ic::time(),
            sender,
            dst_address: address.assume_checked().to_string(),
            status: WithdrawalStatus::Scheduled,
        }
    }

    fn invalid(reason: String) -> Self {
        Self {
            rune_info: RuneInfo::invalid(),
            amount: 0,
            request_ts: 0,
            sender: Default::default(),
            dst_address: "".to_string(),
            status: WithdrawalStatus::InvalidRequest(reason),
        }
    }

    fn dst_address(&self) -> Address {
        // We assume the address to be valid as we only create this struct with checked `new` method.
        // We cannot store `Address` as is as it is not `CandidType`.
        Address::from_str(&self.dst_address)
            .expect("invalid dst address")
            .assume_checked()
    }

    fn with_status(self, status: WithdrawalStatus) -> Self {
        Self { status, ..self }
    }

    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            WithdrawalStatus::TxSent { .. } | WithdrawalStatus::InvalidRequest(_)
        )
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum WithdrawalStatus {
    InvalidRequest(String),
    Scheduled,
    TxSigned { transaction: DidTransaction },
    TxSent { transaction: DidTransaction },
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

pub(crate) struct Withdrawal<UTXO: UtxoProvider> {
    state: Rc<RefCell<State>>,
    utxo_provider: UTXO,
    signer: BtcSignerType,
    network: Network,
    operation_store: RuneOperationStore,
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
            operation_store: get_operations_store(),
        }
    }
}

impl<UTXO: UtxoProvider> Withdrawal<UTXO> {
    pub async fn withdraw(
        &mut self,
        operation_id: MinterOperationId,
    ) -> Result<Txid, WithdrawError> {
        let Some(operation) = self.operation_store.get(operation_id) else {
            return Err(WithdrawError::InternalError(format!(
                "Operation not found: {operation_id}"
            )));
        };

        let OperationState::Withdrawal(ref payload) = operation else {
            return Err(WithdrawError::InternalError(format!(
                "Operation {operation_id} is not a withdrawal operation: {operation:?}"
            )));
        };

        let dst_address = payload.dst_address();

        let RuneWithdrawalPayload {
            rune_info,
            amount,
            sender,
            status,
            ..
        } = payload.clone();

        if !matches!(status, WithdrawalStatus::Scheduled) {
            return Err(WithdrawError::InternalError(format!("Attempted to initiate withdrawal flow for operation {operation_id} but it was not in `Scheduled` state: {operation:?}")));
        }

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
                rune_info.id(),
                utxos.clone(),
            )
            .await?;

        self.operation_store.update(
            operation_id,
            OperationState::Withdrawal(payload.clone().with_status(WithdrawalStatus::TxSigned {
                transaction: DidTransaction(tx.clone()),
            })),
        );

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

        self.operation_store.update(
            operation_id,
            OperationState::Withdrawal(payload.clone().with_status(WithdrawalStatus::TxSent {
                transaction: DidTransaction(tx.clone()),
            })),
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
