use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::consensus::{Decodable, Encodable};
use bitcoin::hashes::Hash;
use bitcoin::{Address, FeeRate, Network, Transaction};
use bridge_did::id256::Id256;
use bridge_utils::bft_events::BurntEventData;
use candid::types::{Serializer, Type};
use candid::{CandidType, Deserialize};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ic_exports::ic_kit::ic;
use ord_rs::wallet::TxInputInfo;
use serde::{Deserializer, Serialize};

use crate::brc20_info::{Brc20Info, Brc20Tick};
use crate::canister::get_brc20_state;
use crate::constants::FEE_RATE_UPDATE_INTERVAL;
use crate::core::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::interface::WithdrawError;
use crate::key::{get_derivation_path_ic, BtcSignerType};
use crate::state::Brc20State;

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct Brc20WithdrawalPayload {
    pub brc20_info: Brc20Info,
    pub amount: u128,
    pub request_ts: u64,
    pub sender: H160,
    pub dst_address: String,
}

impl Brc20WithdrawalPayload {
    pub fn new(
        burnt_event_data: BurntEventData,
        state: &Brc20State,
    ) -> Result<Self, WithdrawError> {
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

        let brc20_tick = Brc20Tick::from(token_id);

        let Some(brc20_info) = state.brc20_info(&brc20_tick) else {
            // We don't need to request the list from the indexer at this point. This operation is
            // called only when some tokens are burned, which means they have been minted before,
            // and that means that we already received the rune info from the indexer.
            return Err(WithdrawError::InvalidRequest(format!(
                "Invalid rune id: {brc20_tick}. No such rune id in the rune list received from the indexer."
            )));
        };

        Ok(Self {
            amount,
            brc20_info,
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
    state: Rc<RefCell<Brc20State>>,
    utxo_provider: UTXO,
    signer: BtcSignerType,
    network: Network,
}

impl Withdrawal<IcUtxoProvider> {
    pub fn new(state: Rc<RefCell<Brc20State>>) -> Self {
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
        Self::new(get_brc20_state())
    }
}

impl<UTXO: UtxoProvider> Withdrawal<UTXO> {
    pub async fn create_withdrawal_transaction(
        &self,
        _payload: Brc20WithdrawalPayload,
    ) -> Result<Transaction, WithdrawError> {
        todo!()
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
            change_utxo,
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

    /// Build a withdrawal transaction.
    async fn build_withdraw_transaction(
        &self,
        args: WithdrawalTransactionArgs,
    ) -> Result<Transaction, WithdrawError> {
        if args.inputs.is_empty() {
            return Err(WithdrawError::NoInputs);
        }

        todo!();
    }

    async fn get_change_address(&self) -> Address {
        self.get_transit_address(&H160::default()).await
    }

    fn get_change_derivation_path(&self) -> Vec<Vec<u8>> {
        get_derivation_path_ic(&H160::default())
    }

    /// Get current fee rate, otherwise if too old, request a new one from the utxo provider.
    async fn get_fee_rate(&self) -> Result<FeeRate, WithdrawError> {
        let (current_fee_rate, elapsed_since_last_fee_rate_update) = {
            let state_ref = self.state.borrow();
            (
                state_ref.fee_rate(),
                state_ref.last_fee_rate_update_elapsed(),
            )
        };

        if elapsed_since_last_fee_rate_update > FEE_RATE_UPDATE_INTERVAL {
            let fee_rate = self.utxo_provider.get_fee_rate().await?;
            let mut state_ref = self.state.borrow_mut();
            state_ref.update_fee_rate(fee_rate);

            Ok(fee_rate)
        } else {
            Ok(current_fee_rate)
        }
    }

    /// Get the minimum amount of utxos that must be used to fund the transaction.
    ///
    /// Returns the utxos that can fund the transaction with the minimum number of utxos.
    /// If the transaction cannot be funded with the minimum number of utxos, the function will return None.
    fn get_greedy_funding_utxos(&self, _args: GetGreedyFundingUtxosArgs) -> Option<Vec<Utxo>> {
        todo!()
    }
}

/// Arguments for the `get_greedy_funding_utxos` function.
struct GetGreedyFundingUtxosArgs {
    rune_utxos_count: usize,
    funding_utxos: Vec<Utxo>,
    rune_change_address: Address,
    destination_address: Address,
    change_address: Address,
    fee_rate: FeeRate,
}

/// Arguments for the `build_withdraw_transaction` function.
struct WithdrawalTransactionArgs {
    tick: Brc20Tick,
    amount: u128,
    dst_address: Address,
    change_address: Address,
    inputs: Vec<TxInputInfo>,
    fee_rate: FeeRate,
    rune_change_address: Address,
}
