use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::bip32::DerivationPath;
use bitcoin::hashes::Hash as _;
use bitcoin::secp256k1::ThirtyTwoByteHash;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, FeeRate, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Txid, Witness,
};
use bridge_did::brc20_info::{Brc20Info, Brc20Tick};
use bridge_did::event_data::BurntEventData;
use bridge_did::id256::Id256;
use bridge_did::operations::{Brc20WithdrawalPayload, DidTransaction, RevealUtxo};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_exports::ic_kit::ic;
use ord_rs::fees::estimate_transaction_fees;
use ord_rs::wallet::{ScriptType, TxInputInfo};
use ord_rs::{
    Brc20, CreateCommitTransaction, CreateCommitTransactionArgs, OrdError, OrdTransactionBuilder,
    RevealTransactionArgs, SignCommitTransactionArgs,
};

use super::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::canister::{get_brc20_state, get_runtime_state};
use crate::constants::FEE_RATE_UPDATE_INTERVAL;
use crate::interface::WithdrawError;
use crate::key::{get_derivation_path, BtcSignerType};
use crate::ledger::UtxoKey;
use crate::state::Brc20State;

pub struct Brc20Transactions {
    pub commit_tx: Transaction,
    pub reveal_tx: Transaction,
    pub reveal_utxo: RevealUtxo,
}

pub fn new_withdraw_payload(
    burnt_event_data: BurntEventData,
    state: &Brc20State,
) -> Result<Brc20WithdrawalPayload, WithdrawError> {
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

    Ok(Brc20WithdrawalPayload {
        amount,
        brc20_info,
        request_ts: ic::time(),
        sender,
        dst_address: address.assume_checked().to_string(),
    })
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
            amount,
            brc20_info: Brc20Info { tick, decimals },
            ..
        } = payload;

        let fee_rate = self.get_fee_rate().await?;
        let funding_address = self.get_funding_address(&sender).await?;
        let reveal_recipient_address = funding_address.clone();
        log::debug!("funding address: {funding_address}; reveal recipient address: {reveal_recipient_address}");
        let amount = Self::convert_erc20_amount_to_brc20(amount, decimals)?;

        // get funding utxos, but filter out input utxos
        let funding_utxos = self.get_funding_utxos(&funding_address).await?;

        // make brc20 transfer inscription
        let derivation_path = get_derivation_path(&sender)?;
        let mut inscriber = self.get_inscriber(&derivation_path).await?;
        let CommitTransaction {
            create_commit_transaction,
            inputs: funding_tx_inputs,
        } = self
            .build_commit_transaction(BuildCommitTransactionArgs {
                inscriber: &mut inscriber,
                funding_utxos,
                tick,
                amount,
                wallet_address: funding_address.clone(),
                fee_rate,
                derivation_path: &derivation_path,
            })
            .await?;

        let signed_commit_tx = inscriber
            .sign_commit_transaction(
                create_commit_transaction.unsigned_tx,
                SignCommitTransactionArgs {
                    inputs: funding_tx_inputs.clone(),
                    txin_script_pubkey: funding_address.script_pubkey(),
                    derivation_path: Some(derivation_path.clone()),
                },
            )
            .await
            .map_err(|e| WithdrawError::TransactionSigning(e.to_string()))?;

        log::info!(
            "Built commit transaction with id {}",
            signed_commit_tx.txid()
        );
        log::debug!("Commit transaction: {signed_commit_tx:?}");

        // make reveal transaction
        let reveal_transaction = inscriber
            .build_reveal_transaction(RevealTransactionArgs {
                input: ord_rs::Utxo {
                    id: signed_commit_tx.txid(),
                    index: 0,
                    amount: create_commit_transaction.reveal_balance,
                },
                recipient_address: reveal_recipient_address,
                redeem_script: create_commit_transaction.redeem_script,
                derivation_path: Some(derivation_path),
            })
            .await
            .map_err(|e| WithdrawError::RevealTransactionError(e.to_string()))?;

        log::info!(
            "Built reveal transaction with id {}",
            reveal_transaction.txid()
        );
        log::debug!("Reveal transaction: {reveal_transaction:?}");

        // get the reveal utxo
        let reveal_utxo = RevealUtxo {
            txid: reveal_transaction.txid().as_raw_hash().into_32(),
            vout: 0,
            value: reveal_transaction.output[0].value.to_sat(),
        };
        log::debug!(
            "Reveal utxo: txid: {}; vout: {}; value: {}",
            reveal_transaction.txid(),
            reveal_utxo.vout,
            reveal_utxo.value
        );

        // store the reveal utxo in the ledger
        {
            let mut state_ref = self.state.borrow_mut();
            state_ref
                .ledger_mut()
                .deposit_reveal(reveal_transaction.txid(), reveal_utxo.vout);
        }

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

    pub fn mark_reveal_utxo_as_used(&self, outpoint: &OutPoint) {
        let mut state_ref = self.state.borrow_mut();
        state_ref.ledger_mut().remove_reveal_utxo(&UtxoKey {
            tx_id: outpoint.txid.clone().as_raw_hash().into_32(),
            vout: outpoint.vout,
        });
    }

    /// Await inscription reveal transaction
    pub async fn await_inscription_transactions(
        &self,
        sender_address: &H160,
        reveal_utxo: RevealUtxo,
    ) -> Result<Utxo, WithdrawError> {
        let reveal_recipient_address = self.get_funding_address(sender_address).await?;
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
        let Brc20WithdrawalPayload {
            dst_address,
            sender,
            ..
        } = payload;

        let funding_address = self.get_funding_address(&sender).await?;
        let fee_rate = self.get_fee_rate().await?;

        let Ok(dst_address) = Address::from_str(&dst_address) else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode recipient address from string: {dst_address}"
            )));
        };
        let dst_address = dst_address.assume_checked();

        // get greedy funding utxos
        let funding_utxos = self
            .get_greedy_funding_utxos(GetGreedyFundingUtxosArgs {
                funding_address: funding_address.clone(),
                recipient_address: dst_address.clone(),
                fee_rate,
            })
            .await?
            .ok_or(WithdrawError::InsufficientFunds)?;

        log::info!("Funding utxos: {}", funding_utxos.len());
        log::debug!("Funding utxos: {funding_utxos:?}");

        // build transaction
        let unsigned_tx =
            self.build_unsigned_transfer_transaction(&reveal_utxo, &funding_utxos, dst_address)?;

        // get transaction input info
        let funding_dp = get_derivation_path(&sender)?;
        let tx_input_info = self.transfer_tx_input_info(
            &reveal_utxo,
            &funding_utxos,
            &funding_dp,
            &funding_address,
        )?;

        log::debug!("Transfer Transaction input info: {tx_input_info:?}");

        // sign transaction
        self.sign_transfer_transaction(unsigned_tx, &tx_input_info)
            .await
            .map(DidTransaction)
    }

    /// Build commit transaction
    async fn build_commit_transaction(
        &self,
        args: BuildCommitTransactionArgs<'_>,
    ) -> Result<CommitTransaction, WithdrawError> {
        let BuildCommitTransactionArgs {
            inscriber,
            mut funding_utxos,
            tick,
            amount,
            wallet_address,
            fee_rate,
            derivation_path,
        } = args;
        let mut input_count = 1;
        // sort the utxos by value; descending
        funding_utxos.sort_by(|a, b| b.value.cmp(&a.value));
        // convert utxos to ord_utxos
        let funding_utxos: Vec<ord_rs::Utxo> = funding_utxos
            .iter()
            .filter_map(|utxo| {
                Some(ord_rs::Utxo {
                    id: Txid::from_slice(&utxo.outpoint.txid).ok()?,
                    index: utxo.outpoint.vout,
                    amount: Amount::from_sat(utxo.value),
                })
            })
            .collect();

        // try to fund the transaction with the minimum number of utxos
        while input_count <= funding_utxos.len() {
            let inscription = Brc20::transfer(tick, amount);
            let inputs: Vec<_> = funding_utxos[0..input_count].to_vec();

            log::info!("input_utxos utxos: {}", inputs.len());
            log::debug!("input_utxos: {inputs:?}");

            match inscriber
                .build_commit_transaction(
                    self.network,
                    wallet_address.clone(),
                    CreateCommitTransactionArgs {
                        inputs: inputs.clone(),
                        inscription,
                        leftovers_recipient: wallet_address.clone(),
                        txin_script_pubkey: wallet_address.script_pubkey(),
                        fee_rate,
                        multisig_config: None,
                        derivation_path: Some(derivation_path.clone()),
                    },
                )
                .await
            {
                Ok(commit_tx) => {
                    return Ok(CommitTransaction {
                        create_commit_transaction: commit_tx,
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
    ) -> Result<Transaction, WithdrawError> {
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
        let tx_out = vec![TxOut {
            value: out_value,
            script_pubkey: recipient_address.script_pubkey(),
        }];

        Ok(Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: tx_in,
            output: tx_out,
        })
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
        derivation_path: &DerivationPath,
        funding_address: &Address,
    ) -> Result<Vec<TxInputInfo>, WithdrawError> {
        let mut tx_input_info = Vec::with_capacity(funding_utxos.len() + 1);
        // push reveal utxo
        tx_input_info.push(TxInputInfo {
            outpoint: OutPoint {
                txid: Txid::from_slice(&reveal_utxo.outpoint.txid).unwrap(),
                vout: reveal_utxo.outpoint.vout,
            },
            tx_out: TxOut {
                value: Amount::from_sat(reveal_utxo.value),
                script_pubkey: funding_address.script_pubkey(),
            },
            derivation_path: derivation_path.clone(),
        });

        for utxo in funding_utxos {
            tx_input_info.push(TxInputInfo {
                outpoint: OutPoint {
                    txid: Txid::from_slice(&utxo.outpoint.txid).unwrap(),
                    vout: utxo.outpoint.vout,
                },
                tx_out: TxOut {
                    value: Amount::from_sat(utxo.value),
                    script_pubkey: funding_address.script_pubkey(),
                },
                derivation_path: derivation_path.clone(),
            });
        }

        Ok(tx_input_info)
    }

    /// Get the BRC20 inscriber for the transaction
    async fn get_inscriber(
        &self,
        derivation_path: &DerivationPath,
    ) -> Result<OrdTransactionBuilder, WithdrawError> {
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

        let public_key = wallet
            .signer
            .ecdsa_public_key(derivation_path)
            .await
            .map_err(|e| WithdrawError::FailedToGetPubkey(e.to_string()))?;

        log::debug!("Ord transaction builder pubkey: {public_key}");

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
    async fn get_greedy_funding_utxos(
        &self,
        args: GetGreedyFundingUtxosArgs,
    ) -> Result<Option<Vec<Utxo>>, WithdrawError> {
        let mut funding_utxos = self.get_funding_utxos(&args.funding_address).await?;
        let mut utxos_count = 1;
        // sort the utxos by value; descending
        funding_utxos.sort_by(|a, b| b.value.cmp(&a.value));

        // try to fund the transaction with the minimum number of utxos
        while utxos_count <= funding_utxos.len() {
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
            let solution = &funding_utxos[0..utxos_count];
            let solution_value =
                Amount::from_sat(solution.iter().map(|utxo| utxo.value).sum::<u64>());
            if solution_value >= required_fee {
                log::debug!("Found a funding solution with {utxos_count} utxos; required fee {required_fee}; fee funds: {solution_value}");
                return Ok(Some(solution.to_vec()));
            }

            // otherwise try with more utxos
            utxos_count += 1;
        }

        Ok(None)
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

    /// Get utxos available for funding the transaction
    ///
    /// It will discard utxos that are reveal utxos
    async fn get_funding_utxos(&self, address: &Address) -> Result<Vec<Utxo>, WithdrawError> {
        let utxos = self
            .utxo_provider
            .get_utxos(address)
            .await
            .map(|utxos| utxos.utxos)
            .map_err(|_| WithdrawError::NoInputs)?;

        let state_ref = self.state.borrow();
        let ledger = state_ref.ledger();

        Ok(utxos
            .into_iter()
            .filter(|utxo| {
                !ledger.reveal_utxos_contains(&UtxoKey {
                    tx_id: utxo.outpoint.txid.clone().try_into().unwrap(),
                    vout: utxo.outpoint.vout,
                })
            })
            .collect())
    }
}

/// Arguments for the `get_greedy_funding_utxos` function.
struct GetGreedyFundingUtxosArgs {
    /// The address that will be used to fund the transaction
    funding_address: Address,
    /// The address that will receive the funds
    recipient_address: Address,
    /// The fee rate for the transaction
    fee_rate: FeeRate,
}

/// Commit transaction outputs
struct CommitTransaction {
    create_commit_transaction: CreateCommitTransaction,
    /// The funding utxos used to fund the transaction
    inputs: Vec<ord_rs::Utxo>,
}

/// Arguments for the `build_commit_transaction` function
struct BuildCommitTransactionArgs<'a> {
    inscriber: &'a mut OrdTransactionBuilder,
    /// The funding utxos
    funding_utxos: Vec<Utxo>,
    /// The BRC20 tick
    tick: Brc20Tick,
    /// The amount to transfer
    amount: u64,
    /// The address that will be used to fund the transaction
    wallet_address: Address,
    /// The fee rate for the transaction
    fee_rate: FeeRate,
    /// The derivation path for the wallet
    derivation_path: &'a DerivationPath,
}

#[cfg(test)]
mod test {

    use bitcoin::PrivateKey;
    use ic_exports::ic_cdk::api::management_canister::bitcoin::{GetUtxosResponse, Outpoint};
    use ic_exports::ic_kit::MockContext;
    use ord_rs::constants::POSTAGE;
    use ord_rs::wallet::LocalSigner;

    use super::*;

    #[test]
    fn test_should_convert_erc20_to_brc20() {
        let amount = 10000000000000000000000;
        let decimals = 18;

        let brc20_amount =
            Withdrawal::<IcUtxoProvider>::convert_erc20_amount_to_brc20(amount, decimals).unwrap();

        assert_eq!(brc20_amount, 10_000);
    }

    #[tokio::test]
    async fn test_should_get_greedy_funding_utxos() {
        let fee_rate = FeeRate::from_sat_per_vb(1000).unwrap();
        let address = Address::from_str("bc1quyc49rn6q9rmlk5rz96pqy8ug827xwvamqm0vh")
            .unwrap()
            .assume_checked();

        // let's calculate the required fee for a transaction with 2 funding utxos
        let fee = estimate_transaction_fees(
            ScriptType::P2WSH,
            2 + 1,
            fee_rate,
            &None,
            vec![TxOut {
                value: Amount::from_sat(POSTAGE),
                script_pubkey: address.script_pubkey(),
            }],
        );

        let first_utxo_value = (fee.to_sat() * 6).div_ceil(10); // 60% of the fee
        let second_utxo_value = (fee.to_sat() * 4).div_ceil(10); // 40% of the fee

        // let's create funding utxos
        let funding_utxos = vec![
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0; 32],
                    vout: 0,
                },
                value: first_utxo_value,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![1; 32],
                    vout: 0,
                },
                value: second_utxo_value,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![2; 32],
                    vout: 0,
                },
                value: 10,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![3; 32],
                    vout: 0,
                },
                value: 10,
                height: 0,
            },
        ];

        let test_provider = TestUtxoProvider {
            utxos: funding_utxos.clone(),
        };

        let state = Rc::new(RefCell::new(Brc20State::default()));
        let withdrawal = Withdrawal {
            state: state.clone(),
            utxo_provider: test_provider,
            signer: BtcSignerType::Local(LocalSigner::new(PrivateKey::generate(Network::Regtest))),
            network: Network::Regtest,
        };

        let greedy_funding_utxos = withdrawal
            .get_greedy_funding_utxos(GetGreedyFundingUtxosArgs {
                funding_address: Address::from_str("bc1quyc49rn6q9rmlk5rz96pqy8ug827xwvamqm0vh")
                    .unwrap()
                    .assume_checked(),
                fee_rate,
                recipient_address: Address::from_str("bc1quyc49rn6q9rmlk5rz96pqy8ug827xwvamqm0vh")
                    .unwrap()
                    .assume_checked(),
            })
            .await
            .unwrap()
            .unwrap();

        assert_eq!(greedy_funding_utxos.len(), 2);
        assert_eq!(greedy_funding_utxos[0].value, first_utxo_value);
        assert_eq!(greedy_funding_utxos[1].value, second_utxo_value);
    }

    #[tokio::test]
    async fn test_should_filter_out_reveal_utxos() {
        MockContext::new().inject();

        let test_provider = TestUtxoProvider {
            utxos: vec![
                Utxo {
                    outpoint: Outpoint {
                        txid: vec![1; 32],
                        vout: 0,
                    },
                    value: 100_000,
                    height: 1,
                },
                Utxo {
                    outpoint: Outpoint {
                        txid: vec![2; 32],
                        vout: 0,
                    },
                    value: 100_000,
                    height: 2,
                },
            ],
        };

        let state = Rc::new(RefCell::new(Brc20State::default()));

        let withdrawal = Withdrawal {
            state: state.clone(),
            utxo_provider: test_provider,
            signer: BtcSignerType::Local(LocalSigner::new(PrivateKey::generate(Network::Regtest))),
            network: Network::Regtest,
        };

        // should return all utxos
        let funding_utxos = withdrawal
            .get_funding_utxos(
                &Address::from_str("bc1quyc49rn6q9rmlk5rz96pqy8ug827xwvamqm0vh")
                    .unwrap()
                    .assume_checked(),
            )
            .await
            .unwrap();

        assert_eq!(funding_utxos.len(), 2);

        // put first utxo in reveal

        {
            let mut state_ref = state.borrow_mut();
            state_ref
                .ledger_mut()
                .deposit_reveal(Txid::from_slice(&[1; 32]).unwrap(), 0);
        }

        // should return only the second utxo
        let funding_utxos = withdrawal
            .get_funding_utxos(
                &Address::from_str("bc1quyc49rn6q9rmlk5rz96pqy8ug827xwvamqm0vh")
                    .unwrap()
                    .assume_checked(),
            )
            .await
            .unwrap();
        assert_eq!(funding_utxos.len(), 1);
        assert_eq!(funding_utxos[0].outpoint.txid, vec![2; 32]);
    }

    struct TestUtxoProvider {
        utxos: Vec<Utxo>,
    }

    impl UtxoProvider for TestUtxoProvider {
        async fn get_utxos(
            &self,
            _address: &Address,
        ) -> Result<GetUtxosResponse, crate::interface::DepositError> {
            Ok(GetUtxosResponse {
                utxos: self.utxos.clone(),
                tip_block_hash: vec![],
                tip_height: 0,
                next_page: None,
            })
        }

        async fn get_fee_rate(&self) -> Result<FeeRate, WithdrawError> {
            unimplemented!()
        }

        async fn send_tx(&self, _transaction: &Transaction) -> Result<(), WithdrawError> {
            Ok(())
        }
    }
}
