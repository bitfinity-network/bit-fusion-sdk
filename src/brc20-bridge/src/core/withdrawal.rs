use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::consensus::{Decodable, Encodable};
use bitcoin::hashes::Hash as _;
use bitcoin::secp256k1::{SecretKey, ThirtyTwoByteHash};
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, FeeRate, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Txid, Witness,
};
use bridge_did::id256::Id256;
use bridge_utils::bft_events::BurntEventData;
use candid::types::{Serializer, Type};
use candid::{CandidType, Deserialize};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_exports::ic_kit::ic;
use ord_rs::fees::estimate_transaction_fees;
use ord_rs::wallet::{ScriptType, TaprootKeypair, TxInputInfo};
use ord_rs::{
    Brc20, CreateCommitTransaction, CreateCommitTransactionArgs, OrdError, OrdTransactionBuilder,
    RevealTransactionArgs, SignCommitTransactionArgs,
};
use serde::{Deserializer, Serialize};

use super::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::brc20_info::{Brc20Info, Brc20Tick};
use crate::canister::{get_brc20_state, get_runtime_state};
use crate::constants::{FEE_RATE_UPDATE_INTERVAL, MAX_TAPROOT_KEYPAIR_GEN_ATTEMPTS};
use crate::interface::WithdrawError;
use crate::key::{get_derivation_path, BtcSignerType};
use crate::state::Brc20State;

pub struct Brc20Transactions {
    pub commit_tx: Transaction,
    pub reveal_tx: Transaction,
    pub reveal_utxo: RevealUtxo,
}

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct RevealUtxo {
    pub txid: [u8; 32],
    pub vout: u32,
    pub value: u64,
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
                "Invalid brc20 id: {brc20_tick}. No such brc20 id in the brc20 list received from the indexer."
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
        let reveal_recipient_address = self.get_change_address().await?;
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

        // make brc20 transfer inscription
        let mut inscriber = self.get_inscriber()?;
        let CommitTransaction {
            args: commit_tx,
            inputs: funding_tx_inputs,
        } = self
            .build_commit_transaction(
                &mut inscriber,
                funding_utxos,
                tick,
                amount,
                funding_address.clone(),
                fee_rate,
            )
            .await?;

        let signed_commit_tx = inscriber
            .sign_commit_transaction(
                commit_tx.unsigned_tx,
                SignCommitTransactionArgs {
                    inputs: funding_tx_inputs.clone(),
                    txin_script_pubkey: funding_address.script_pubkey(),
                },
            )
            .await
            .map_err(|e| WithdrawError::TransactionSigning(e.to_string()))?;

        // make reveal transaction
        let reveal_transaction = inscriber
            .build_reveal_transaction(RevealTransactionArgs {
                input: ord_rs::Utxo {
                    id: signed_commit_tx.txid(),
                    index: 0,
                    amount: commit_tx.reveal_balance,
                },
                recipient_address: reveal_recipient_address,
                redeem_script: commit_tx.redeem_script,
            })
            .await
            .map_err(|e| WithdrawError::RevealTransactionError(e.to_string()))?;

        // mark the funding utxos as used
        {
            let mut state = self.state.borrow_mut();
            let ledger = state.ledger_mut();
            for utxo in funding_tx_inputs {
                ledger.mark_as_used(utxo.into(), dst_address.clone());
            }
        }

        // get the reveal utxo
        let reveal_utxo = RevealUtxo {
            txid: reveal_transaction.txid().as_raw_hash().into_32(),
            vout: 0,
            value: reveal_transaction.output[0].value.to_sat(),
        };

        Ok(Brc20Transactions {
            commit_tx: signed_commit_tx,
            reveal_tx: reveal_transaction,
            reveal_utxo,
        })
    }

    /// Send BTC transaction
    pub async fn send_transaction(&self, tx: Transaction) -> Result<(), WithdrawError> {
        self.utxo_provider.send_tx(&tx).await?;

        Ok(())
    }

    /// Await inscription reveal transaction
    pub async fn await_inscription_transactions(
        &self,
        reveal_utxo: RevealUtxo,
    ) -> Result<Utxo, WithdrawError> {
        let reveal_recipient_address = self.get_change_address().await?;
        let txid = Txid::from_slice(&reveal_utxo.txid).unwrap();
        log::debug!("checking whether the reveal transaction {txid} is confirmed for address {reveal_recipient_address}");
        // get utxos for the reveal address
        self.utxo_provider
            .get_utxos(&reveal_recipient_address)
            .await
            .map_err(|_| WithdrawError::TxNotConfirmed)?
            .utxos
            .into_iter()
            .find(|utxo| {
                utxo.outpoint.txid == reveal_utxo.txid && utxo.outpoint.vout == reveal_utxo.vout
            })
            .ok_or(WithdrawError::TxNotConfirmed)
    }

    /// Build transfer transaction
    ///
    /// The transfer transaction has the following inputs:
    ///
    /// - the reveal utxo, owned by the change address
    /// - the funding utxos, owned by the address associated with the sender
    pub async fn build_transfer_transaction(
        &self,
        payload: Brc20WithdrawalPayload,
        reveal_utxo: Utxo,
    ) -> Result<DidTransaction, WithdrawError> {
        let Brc20WithdrawalPayload { dst_address, .. } = payload;

        let funding_address = self.get_funding_address(&payload.sender).await?;
        let fee_rate = self.get_fee_rate().await?;

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

        // get greedy funding utxos
        let funding_utxos = self
            .get_greedy_funding_utxos(GetGreedyFundingUtxosArgs {
                funding_utxos,
                recipient_address: dst_address.clone(),
                fee_rate,
            })
            .ok_or(WithdrawError::InsufficientFunds)?;

        log::info!("Funding utxos: {}", funding_utxos.len());
        log::debug!("Funding utxos: {funding_utxos:?}");

        // build transaction
        let (unsigned_tx, tx_out) =
            self.build_unsigned_transfer_transaction(&reveal_utxo, &funding_utxos, dst_address)?;

        // get transaction input info
        let tx_input_info =
            self.transfer_tx_input_info(&reveal_utxo, &funding_utxos, &tx_out, &payload.sender)?;

        // sign transaction
        self.sign_transfer_transaction(unsigned_tx, &tx_input_info)
            .await
            .map(DidTransaction)
    }

    /// Build commit transaction
    async fn build_commit_transaction(
        &self,
        inscriber: &mut OrdTransactionBuilder,
        mut funding_utxos: Vec<Utxo>,
        tick: Brc20Tick,
        amount: u64,
        wallet_address: Address,
        fee_rate: FeeRate,
    ) -> Result<CommitTransaction, WithdrawError> {
        let mut input_count = 1;
        // sort the utxos by value; descending
        funding_utxos.sort_by(|a, b| b.value.cmp(&a.value));

        // try to fund the transaction with the minimum number of utxos
        while input_count <= funding_utxos.len() {
            let inscription = Brc20::transfer(tick, amount);
            let inputs: Vec<_> = funding_utxos[0..input_count]
                .iter()
                .filter_map(|utxo| {
                    Some(ord_rs::Utxo {
                        id: Txid::from_slice(&utxo.outpoint.txid).ok()?,
                        index: utxo.outpoint.vout,
                        amount: Amount::from_sat(utxo.value),
                    })
                })
                .collect();

            log::info!("input_utxos utxos: {}", inputs.len());
            log::debug!("input_utxos: {inputs:?}");

            // get random keypair for taproot
            let taproot_keypair = self.get_taproot_keypair().await?;

            match inscriber.build_commit_transaction(
                self.network,
                wallet_address.clone(),
                CreateCommitTransactionArgs {
                    inputs: inputs.clone(),
                    inscription,
                    leftovers_recipient: wallet_address.clone(),
                    txin_script_pubkey: wallet_address.script_pubkey(),
                    fee_rate,
                    multisig_config: None,
                    taproot_keypair: Some(TaprootKeypair::SecretKey(taproot_keypair)),
                },
            ) {
                Ok(commit_tx) => {
                    return Ok(CommitTransaction {
                        args: commit_tx,
                        inputs,
                    });
                }
                Err(OrdError::InsufficientBalance {
                    required,
                    available,
                }) => {
                    log::debug!(
                        "Failed to build commit transaction with {input_count} utxos; required {required}; available {available}, trying with {} utxos", input_count + 1
                    );
                    input_count += 1;
                }
                Err(e) => {
                    log::error!("Failed to build commit transaction: {e}");
                    return Err(WithdrawError::CommitTransactionError(e.to_string()));
                }
            }
        }

        Err(WithdrawError::InsufficientFunds)
    }

    /// Build transfer transaction
    fn build_unsigned_transfer_transaction(
        &self,
        reveal_utxo: &Utxo,
        funding_utxos: &[Utxo],
        recipient_address: Address,
    ) -> Result<(Transaction, TxOut), WithdrawError> {
        let out_value = Amount::from_sat(reveal_utxo.value);
        // build txin
        let mut tx_in = Vec::with_capacity(funding_utxos.len() + 1);
        for utxo in [reveal_utxo].into_iter().chain(funding_utxos) {
            tx_in.push(TxIn {
                previous_output: OutPoint {
                    txid: Txid::from_slice(&utxo.outpoint.txid)
                        .map_err(|_| WithdrawError::InvalidTxid(utxo.outpoint.txid.to_vec()))?,
                    vout: utxo.outpoint.vout,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::from_consensus(0xffffffff),
                witness: Witness::new(),
            });
        }
        // build txout
        let tx_out = TxOut {
            value: out_value,
            script_pubkey: recipient_address.script_pubkey(),
        };

        Ok((
            Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: tx_in,
                output: vec![tx_out.clone()],
            },
            tx_out,
        ))
    }

    /// Sign the transfer transaction
    async fn sign_transfer_transaction(
        &self,
        unsigned_tx: Transaction,
        tx_input_info: &[TxInputInfo],
    ) -> Result<Transaction, WithdrawError> {
        let signer = self
            .state
            .borrow()
            .wallet(
                &get_runtime_state()
                    .borrow()
                    .config
                    .borrow()
                    .get_signing_strategy(),
            )
            .ok_or(WithdrawError::SignerNotInitialized)?;

        signer
            .sign_transaction(&unsigned_tx, tx_input_info)
            .await
            .map_err(|e| WithdrawError::TransactionSigning(e.to_string()))
    }

    /// Get the transaction input info for the transfer transaction
    fn transfer_tx_input_info(
        &self,
        reveal_utxo: &Utxo,
        funding_utxos: &[Utxo],
        tx_out: &TxOut,
        recipient_address: &H160,
    ) -> Result<Vec<TxInputInfo>, WithdrawError> {
        let mut tx_input_info = Vec::with_capacity(funding_utxos.len() + 1);
        // push reveal utxo
        tx_input_info.push(TxInputInfo {
            outpoint: OutPoint {
                txid: Txid::from_slice(&reveal_utxo.outpoint.txid).unwrap(),
                vout: reveal_utxo.outpoint.vout,
            },
            tx_out: tx_out.clone(),
            derivation_path: get_derivation_path(&H160::default())?,
        });

        for utxo in funding_utxos {
            tx_input_info.push(TxInputInfo {
                outpoint: OutPoint {
                    txid: Txid::from_slice(&utxo.outpoint.txid).unwrap(),
                    vout: utxo.outpoint.vout,
                },
                tx_out: tx_out.clone(),
                derivation_path: get_derivation_path(recipient_address)?,
            });
        }

        Ok(tx_input_info)
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
    fn get_greedy_funding_utxos(&self, mut args: GetGreedyFundingUtxosArgs) -> Option<Vec<Utxo>> {
        let mut utxos_count = 1;
        // sort the utxos by value; descending
        args.funding_utxos.sort_by(|a, b| b.value.cmp(&a.value));

        // try to fund the transaction with the minimum number of utxos
        while utxos_count <= args.funding_utxos.len() {
            let required_fee = estimate_transaction_fees(
                ScriptType::P2WSH,
                utxos_count + 1,
                args.fee_rate,
                &None,
                vec![TxOut {
                    value: Amount::ZERO,
                    script_pubkey: args.recipient_address.script_pubkey(),
                }],
            );

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

    /// Convert the ERC20 amount to the BRC20 amount.
    ///
    /// So this basically gets the "integer" amount of the token
    fn convert_erc20_amount_to_brc20(amount: u128, decimals: u8) -> Result<u64, WithdrawError> {
        (amount / 10u128.pow(decimals as u32))
            .try_into()
            .map_err(|_| WithdrawError::AmountTooBig(amount))
    }

    /// Get the BTC address that will be used to fund the transaction.
    async fn get_funding_address(&self, eth_address: &H160) -> Result<Address, WithdrawError> {
        self.signer
            .get_transit_address(eth_address, self.network)
            .await
            .map_err(WithdrawError::from)
    }

    /// Get the change address for the transaction.
    ///
    /// The change address is the address that will receive the reveal tx from the inscription.
    /// This is done in order to prevent spending the reveal transactions
    async fn get_change_address(&self) -> Result<Address, WithdrawError> {
        self.signer
            .get_transit_address(&H160::default(), self.network)
            .await
            .map_err(WithdrawError::from)
    }

    /// Get the public key for the taproot keypair
    ///
    /// The taproot keypair is generated from a random 32 bytes buffer.
    /// The buffer is generated until a valid secret key is created.
    ///
    /// This approach, which seems inefficient, is actually used in the official Bitcoin rust library.
    async fn get_taproot_keypair(&self) -> Result<SecretKey, WithdrawError> {
        for attempt in 1..=MAX_TAPROOT_KEYPAIR_GEN_ATTEMPTS {
            let rand_buff = match ic_exports::ic_cdk::api::management_canister::main::raw_rand()
                .await
            {
                Ok((rand_buff,)) => rand_buff,
                Err((code, message)) => {
                    log::error!(
                        "Failed to get random bytes from management canister: {code:?}: {message}; Attempt {attempt} of {MAX_TAPROOT_KEYPAIR_GEN_ATTEMPTS}"
                    );
                    continue;
                }
            };

            match SecretKey::from_slice(&rand_buff[0..32]) {
                Ok(key) => {
                    log::debug!("generated taproot keypair after {attempt} attempts");
                    return Ok(key);
                }
                Err(e) => {
                    log::debug!("Failed to create secret key from random bytes: {e}; Attempt {attempt} of {MAX_TAPROOT_KEYPAIR_GEN_ATTEMPTS}");
                    continue;
                }
            }
        }

        Err(WithdrawError::FailedToGenerateTaprootKeypair)
    }
}

/// Arguments for the `get_greedy_funding_utxos` function.
struct GetGreedyFundingUtxosArgs {
    funding_utxos: Vec<Utxo>,
    recipient_address: Address,
    fee_rate: FeeRate,
}

/// Commit transaction outputs
struct CommitTransaction {
    args: CreateCommitTransaction,
    inputs: Vec<ord_rs::Utxo>,
}
