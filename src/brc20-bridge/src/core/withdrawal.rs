use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::consensus::{Decodable, Encodable};
use bitcoin::hashes::Hash as _;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, FeeRate, Network, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Txid,
};
use bridge_did::id256::Id256;
use bridge_utils::bft_events::BurntEventData;
use candid::types::{Serializer, Type};
use candid::{CandidType, Deserialize};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_exports::ic_kit::ic;
use ord_rs::fees::{
    estimate_commit_fee, estimate_edict_transaction_fees, estimate_reveal_fee,
    estimate_transaction_fees,
};
use ord_rs::wallet::{CreateCommitTransactionArgsV2, ScriptType, TxInputInfo};
use ord_rs::{Brc20, OrdTransactionBuilder, RevealTransactionArgs, SignCommitTransactionArgs};
use serde::{Deserializer, Serialize};

use super::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::brc20_info::{Brc20Info, Brc20Tick};
use crate::canister::{get_brc20_state, get_runtime_state};
use crate::constants::FEE_RATE_UPDATE_INTERVAL;
use crate::interface::WithdrawError;
use crate::key::{get_derivation_path, BtcSignerType};
use crate::state::Brc20State;

pub struct Brc20Transactions {
    pub commit_tx: Transaction,
    pub reveal_tx: Transaction,
    pub reveal_utxo: Utxo,
}

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
    pub fn new(state: Rc<RefCell<Brc20State>>) -> Result<Self, WithdrawError> {
        let state_ref = state.borrow();

        let signing_strategy = get_runtime_state()
            .borrow()
            .config
            .borrow()
            .get_signing_strategy();

        let network = state_ref.network();
        let ic_network = state_ref.ic_btc_network();
        let signer = state_ref
            .btc_signer(&signing_strategy)
            .ok_or(WithdrawError::SignerNotInitialized)?;

        drop(state_ref);

        Ok(Self {
            state,
            network,
            signer,
            utxo_provider: IcUtxoProvider::new(ic_network),
        })
    }

    pub fn get() -> Result<Self, WithdrawError> {
        Self::new(get_brc20_state())
    }
}

impl<UTXO: UtxoProvider> Withdrawal<UTXO> {
    /// Create BRC20 transfer inscription transactions
    pub async fn build_brc20_transfer_transactions(
        &self,
        payload: Brc20WithdrawalPayload,
    ) -> Result<Brc20Transactions, WithdrawError> {
        let Brc20WithdrawalPayload {
            sender,
            dst_address,
            amount,
            brc20_info: Brc20Info { tick, decimals },
            ..
        } = payload;

        let fee_rate = self.get_fee_rate().await?;
        let funding_address = self.get_funding_address(&sender).await?;
        let amount = Self::convert_erc20_amount_to_brc20(amount, decimals)?;

        // get funding utxos, but filter out input utxos
        let funding_utxos = self
            .utxo_provider
            .get_utxos(&funding_address)
            .await
            .map_err(|_| WithdrawError::NoInputs)?
            .utxos;

        let Ok(dst_address) = Address::from_str(&dst_address) else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode recipient address from string: {dst_address}"
            )));
        };
        let dst_address = dst_address.assume_checked();

        let mut funding_tx_inputs = vec![];
        for utxo in self
            .get_greedy_funding_utxos(
                TransactionType::Commit,
                GetGreedyFundingUtxosArgs {
                    funding_utxos,
                    fee_rate,
                    recipient_address: dst_address.clone(),
                },
            )
            .ok_or(WithdrawError::InsufficientFunds)?
            .into_iter()
        {
            funding_tx_inputs.push(ord_rs::Utxo {
                id: Txid::from_slice(&utxo.outpoint.txid)
                    .map_err(|_| WithdrawError::InvalidTxid(utxo.outpoint.txid))?,
                index: utxo.outpoint.vout,
                amount: Amount::from_sat(utxo.value),
            });
        }

        log::info!("input_utxos utxos: {}", funding_tx_inputs.len());
        log::debug!("input_utxos: {funding_tx_inputs:?}");

        // make brc20 transfer inscription
        let transfer_inscription = Brc20::transfer(tick, amount);
        let mut inscriber = self.get_inscriber()?;

        let commit_tx = inscriber
            .build_commit_transaction_with_fixed_fees(
                self.network,
                CreateCommitTransactionArgsV2 {
                    inputs: funding_tx_inputs.clone(),
                    inscription: transfer_inscription,
                    leftovers_recipient: funding_address.clone(),
                    commit_fee: Amount::from_sat(1000),
                    reveal_fee: Amount::from_sat(1000),
                    txin_script_pubkey: funding_address.script_pubkey(),
                },
            )
            .map_err(|e| WithdrawError::CommitTransactionError(e.to_string()))?;

        let signed_commit_tx = inscriber
            .sign_commit_transaction(
                commit_tx.unsigned_tx,
                SignCommitTransactionArgs {
                    inputs: funding_tx_inputs.clone(),
                    txin_script_pubkey: funding_address.script_pubkey(),
                },
            )
            .await
            .map_err(|_| WithdrawError::TransactionSigning)?;

        // make reveal transaction
        let reveal_transaction = inscriber
            .build_reveal_transaction(RevealTransactionArgs {
                input: ord_rs::Utxo {
                    id: signed_commit_tx.txid(),
                    index: 0,
                    amount: commit_tx.reveal_balance,
                },
                recipient_address: funding_address.clone(),
                redeem_script: commit_tx.redeem_script,
            })
            .await
            .map_err(|e| WithdrawError::RevealTransactionError(e.to_string()))?;

        {
            let mut state = self.state.borrow_mut();
            let ledger = state.ledger_mut();
            for utxo in funding_tx_inputs {
                ledger.mark_as_used(utxo.into(), dst_address.clone());
            }
        }

        todo!();
    }

    fn get_inscriber(&self) -> Result<OrdTransactionBuilder, WithdrawError> {
        let public_key = self
            .state
            .borrow()
            .public_key()
            .ok_or(WithdrawError::SignerNotInitialized)?;

        let signing_strategy = get_runtime_state()
            .borrow()
            .config
            .borrow()
            .get_signing_strategy();

        let wallet = self
            .state
            .borrow()
            .wallet(&signing_strategy)
            .ok_or(WithdrawError::SignerNotInitialized)?;

        Ok(OrdTransactionBuilder::new(
            public_key,
            ScriptType::P2TR,
            wallet,
        ))
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
    fn get_greedy_funding_utxos(
        &self,
        tx_type: TransactionType,
        mut args: GetGreedyFundingUtxosArgs,
    ) -> Option<Vec<Utxo>> {
        let mut utxos_count = 1;
        // sort the utxos by value; descending
        args.funding_utxos.sort_by(|a, b| b.value.cmp(&a.value));

        // try to fund the transaction with the minimum number of utxos
        while utxos_count <= args.funding_utxos.len() {
            let required_fee = tx_type.required_fee(&args, utxos_count);

            // find the minimum number of utxos that can fund the transaction
            let solution = &args.funding_utxos[0..utxos_count];
            let solution_value =
                Amount::from_sat(solution.iter().map(|utxo| utxo.value).sum::<u64>());
            if solution_value >= required_fee {
                log::debug!("Found a funding solution with {utxos_count} utxos; required fee {required_fee}; fee funds: {solution_value}");
                return Some(solution.to_vec());
            }

            // otherwise try with more utxos
            utxos_count += 1;
        }

        None
    }

    /// Get the BTC address that will be used to fund the transaction.
    async fn get_funding_address(&self, eth_address: &H160) -> Result<Address, WithdrawError> {
        self.signer
            .get_transit_address(eth_address, self.network)
            .await
            .map_err(WithdrawError::from)
    }

    /// Convert the ERC20 amount to the BRC20 amount.
    ///
    /// So this basically gets the "integer" amount of the token
    fn convert_erc20_amount_to_brc20(amount: u128, decimals: u8) -> Result<u64, WithdrawError> {
        (amount / 10u128.pow(decimals as u32))
            .try_into()
            .map_err(|_| WithdrawError::AmountTooBig(amount))
    }
}

/// Arguments for the `get_greedy_funding_utxos` function.
struct GetGreedyFundingUtxosArgs {
    funding_utxos: Vec<Utxo>,
    recipient_address: Address,
    fee_rate: FeeRate,
}

/// Transaction type to estimate
enum TransactionType {
    Commit,
    Transfer,
}

impl TransactionType {
    fn required_fee(&self, args: &GetGreedyFundingUtxosArgs, fund_utxos_count: usize) -> Amount {
        match self {
            Self::Commit => {
                let dummy_tx = Transaction {
                    version: Version::TWO,
                    lock_time: LockTime::ZERO,
                    input: vec![TxIn::default(); fund_utxos_count],
                    output: vec![
                        TxOut {
                            value: Amount::ZERO,
                            script_pubkey: args.recipient_address.script_pubkey(),
                        };
                        2
                    ],
                };
                estimate_commit_fee(dummy_tx, ScriptType::P2WSH, args.fee_rate, &None) * 2
            }
            Self::Transfer => estimate_transaction_fees(
                ScriptType::P2WSH,
                fund_utxos_count + 1,
                args.fee_rate,
                &None,
                vec![TxOut {
                    value: Amount::ZERO,
                    script_pubkey: args.recipient_address.script_pubkey(),
                }],
            ),
        }
    }
}
