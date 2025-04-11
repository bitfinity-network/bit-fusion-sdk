use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::hashes::Hash;
use bitcoin::{Address, Amount, FeeRate, Network, OutPoint, Transaction, TxOut, Txid};
use bridge_did::event_data::BurntEventData;
use bridge_did::id256::Id256;
use bridge_did::runes::RuneWithdrawalPayload;
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ic_exports::ic_kit::ic;
use ord_rs::OrdTransactionBuilder;
use ord_rs::fees::{EstimateEdictTxFeesArgs, estimate_edict_transaction_fees};
use ord_rs::wallet::{CreateEdictTxArgs, ScriptType, TxInputInfo};
use ordinals::RuneId;

use crate::canister::{get_rune_state, get_runtime_state};
use crate::constants::FEE_RATE_UPDATE_INTERVAL;
use crate::core::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::interface::WithdrawError;
use crate::key::{BtcSignerType, get_derivation_path, get_derivation_path_ic};
use crate::state::RuneState;

pub struct RuneWithdrawalPayloadImpl(pub RuneWithdrawalPayload);

impl RuneWithdrawalPayloadImpl {
    pub fn new(burnt_event_data: BurntEventData, state: &RuneState) -> Result<Self, WithdrawError> {
        let BurntEventData {
            recipient_id,
            amount,
            to_token,
            sender,
            ..
        } = burnt_event_data;

        let amount: u128 = amount.0.to();

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

        Ok(Self(RuneWithdrawalPayload {
            rune_info,
            amount,
            request_ts: ic::time(),
            sender,
            dst_address: address.assume_checked().to_string(),
        }))
    }
}

pub(crate) struct Withdrawal<UTXO: UtxoProvider> {
    state: Rc<RefCell<RuneState>>,
    utxo_provider: UTXO,
    signer: BtcSignerType,
    network: Network,
}

impl Withdrawal<IcUtxoProvider> {
    pub fn new(state: Rc<RefCell<RuneState>>) -> Result<Self, WithdrawError> {
        let state_ref = state.borrow();

        let signing_strategy = get_runtime_state()
            .borrow()
            .config
            .borrow()
            .get_signing_strategy();

        let network = state_ref.network();
        let ic_network = state_ref.ic_btc_network();
        let cache_timeout = state_ref.utxo_cache_timeout();
        let signer = state_ref
            .btc_signer(&signing_strategy)
            .ok_or(WithdrawError::SignerNotInitialized)?;

        drop(state_ref);

        Ok(Self {
            state,
            network,
            signer,
            utxo_provider: IcUtxoProvider::new(ic_network, cache_timeout),
        })
    }

    pub fn get() -> Result<Self, WithdrawError> {
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

        let fee_rate = self.get_fee_rate().await?;

        let unspent_utxo_info = self.state.borrow().ledger().load_unspent_utxos()?;
        let funding_address = self.get_transit_address(&sender).await?;
        let rune_change_address = self.get_change_address().await?;

        // get rune utxos
        let mut input_utxos: Vec<_> = unspent_utxo_info
            .into_values()
            .filter(|info| info.rune_info.contains(&rune_info))
            .map(|info| info.tx_input_info)
            .collect();

        // if there are no utxos, return an error
        if input_utxos.is_empty() {
            return Err(WithdrawError::NoInputs);
        }

        // get funding utxos, but filter out input utxos
        let funding_utxos = self
            .utxo_provider
            .get_utxos(&funding_address)
            .await
            .map_err(|_| WithdrawError::NoInputs)?
            .utxos
            .into_iter()
            .filter(|utxo| {
                input_utxos.iter().all(|input| {
                    input.outpoint.txid != Txid::from_slice(&utxo.outpoint.txid).unwrap()
                        && input.outpoint.vout != utxo.outpoint.vout
                })
            })
            .collect();

        let Ok(dst_address) = Address::from_str(&dst_address) else {
            return Err(WithdrawError::InvalidRequest(format!(
                "Failed to decode recipient address from string: {dst_address}"
            )));
        };
        let dst_address = dst_address.assume_checked();

        // Get the utxos that can fund the transaction with the minimum number of utxos
        let mut funding_tx_inputs = vec![];
        for utxo in self
            .get_greedy_funding_utxos(GetGreedyFundingUtxosArgs {
                rune_utxos_count: input_utxos.len(),
                funding_utxos,
                rune_change_address: rune_change_address.clone(),
                destination_address: dst_address.clone(),
                change_address: funding_address.clone(),
                rune: rune_info.id(),
                rune_amount: amount,
                fee_rate,
            })
            .ok_or(WithdrawError::InsufficientFunds)?
            .into_iter()
        {
            funding_tx_inputs.push(TxInputInfo {
                outpoint: OutPoint {
                    txid: Txid::from_slice(&utxo.outpoint.txid).unwrap(),
                    vout: utxo.outpoint.vout,
                },
                tx_out: TxOut {
                    value: Amount::from_sat(utxo.value),
                    script_pubkey: funding_address.script_pubkey(),
                },
                derivation_path: get_derivation_path(&sender)?,
            });
        }

        input_utxos.extend(funding_tx_inputs);
        log::info!("input_utxos utxos: {}", input_utxos.len());
        log::debug!("input_utxos: {input_utxos:?}");

        let tx = self
            .build_withdraw_transaction(WithdrawalTransactionArgs {
                change_address: funding_address,
                dst_address: dst_address.clone(),
                fee_rate,
                inputs: input_utxos.clone(),
                rune_change_address,
                runes: vec![(rune_info.id(), amount)],
            })
            .await?;

        {
            let mut state = self.state.borrow_mut();
            let ledger = state.ledger_mut();
            for utxo in input_utxos {
                ledger.mark_as_used(utxo.outpoint.into(), dst_address.clone());
            }
        }

        Ok(tx)
    }

    pub async fn send_transaction(&self, tx: Transaction) -> Result<(), WithdrawError> {
        self.utxo_provider.send_tx(&tx).await?;
        let change_address = self.get_change_address().await?;

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
            vec![],
        );

        Ok(())
    }

    async fn get_transit_address(&self, eth_address: &H160) -> Result<Address, WithdrawError> {
        self.signer
            .get_transit_address(eth_address, self.network)
            .await
            .map_err(WithdrawError::from)
    }

    /// Build a withdrawal transaction.
    async fn build_withdraw_transaction(
        &self,
        args: WithdrawalTransactionArgs,
    ) -> Result<Transaction, WithdrawError> {
        if args.inputs.is_empty() {
            return Err(WithdrawError::NoInputs);
        }

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

        let builder = OrdTransactionBuilder::new(public_key, ScriptType::P2WSH, wallet);

        let args = CreateEdictTxArgs {
            runes: args.runes,
            inputs: args.inputs,
            destination: args.dst_address,
            change_address: args.change_address,
            rune_change_address: args.rune_change_address,
            fee_rate: args.fee_rate,
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

    async fn get_change_address(&self) -> Result<Address, WithdrawError> {
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
    fn get_greedy_funding_utxos(&self, mut args: GetGreedyFundingUtxosArgs) -> Option<Vec<Utxo>> {
        let mut utxos_count = 1;
        // sort the utxos by value; descending
        args.funding_utxos.sort_by(|a, b| b.value.cmp(&a.value));

        // try to fund the transaction with the minimum number of utxos
        while utxos_count <= args.funding_utxos.len() {
            let required_fee = estimate_edict_transaction_fees(EstimateEdictTxFeesArgs {
                script_type: ScriptType::P2WSH,
                number_of_inputs: utxos_count + args.rune_utxos_count,
                current_fee_rate: args.fee_rate,
                multisig_config: None,
                rune_change_address: args.rune_change_address.clone(),
                destination_address: args.destination_address.clone(),
                change_address: args.change_address.clone(),
                rune: args.rune,
                rune_amount: args.rune_amount,
            });

            // find the minimum number of utxos that can fund the transaction
            let solution = &args.funding_utxos[0..utxos_count];
            let solution_value =
                Amount::from_sat(solution.iter().map(|utxo| utxo.value).sum::<u64>());
            if solution_value >= required_fee {
                log::debug!(
                    "Found a funding solution with {utxos_count} utxos; required fee {required_fee}; fee funds: {solution_value}"
                );
                return Some(solution.to_vec());
            }

            // otherwise try with more utxos
            utxos_count += 1;
        }

        None
    }
}

/// Arguments for the `get_greedy_funding_utxos` function.
struct GetGreedyFundingUtxosArgs {
    rune_utxos_count: usize,
    funding_utxos: Vec<Utxo>,
    rune_change_address: Address,
    destination_address: Address,
    change_address: Address,
    rune: RuneId,
    rune_amount: u128,
    fee_rate: FeeRate,
}

/// Arguments for the `build_withdraw_transaction` function.
struct WithdrawalTransactionArgs {
    runes: Vec<(RuneId, u128)>,
    dst_address: Address,
    change_address: Address,
    inputs: Vec<TxInputInfo>,
    fee_rate: FeeRate,
    rune_change_address: Address,
}

#[cfg(test)]
mod test {
    use bitcoin::{Address, FeeRate, PrivateKey, Transaction};
    use ic_exports::ic_cdk::api::management_canister::bitcoin::GetUtxosResponse;
    use ic_exports::ic_kit::MockContext;
    use ord_rs::wallet::LocalSigner;

    use super::*;
    use crate::core::rune_inputs::GetInputsError;
    use crate::core::utxo_provider::UtxoProvider;
    use crate::interface::WithdrawError;
    use crate::key::BtcSignerType;
    use crate::state::RuneState;

    #[tokio::test]
    async fn test_should_get_fee_rate() {
        MockContext::new().inject();

        let mut withdrawal = test_withdrawal();

        // First call should return the fee rate from the provider
        let fee_rate = withdrawal.get_fee_rate().await.unwrap();
        assert_eq!(fee_rate, FeeRate::from_sat_per_vb(1).unwrap());

        // update the fee rate in the provider
        withdrawal.utxo_provider = FakeUtxoProvider {
            fee_rate: FeeRate::from_sat_per_vb(2).unwrap(),
        };

        // Second call should return the fee rate from the state
        let fee_rate = withdrawal.get_fee_rate().await.unwrap();
        assert_eq!(fee_rate, FeeRate::from_sat_per_vb(1).unwrap());
    }

    struct FakeUtxoProvider {
        fee_rate: FeeRate,
    }

    impl UtxoProvider for FakeUtxoProvider {
        async fn get_utxos(&self, _address: &Address) -> Result<GetUtxosResponse, GetInputsError> {
            unimplemented!()
        }

        async fn get_fee_rate(&self) -> Result<FeeRate, WithdrawError> {
            Ok(self.fee_rate)
        }

        async fn send_tx(&self, _transaction: &Transaction) -> Result<(), WithdrawError> {
            unimplemented!()
        }
    }

    #[test]
    fn test_should_get_greedy_funding_utxos() {
        let rune = RuneId::new(219, 1).unwrap();
        let rune_amount = 9500;

        let fee_rate = FeeRate::from_sat_per_vb(1).unwrap();
        let address =
            Address::from_str("bc1pxwww0ct9ue7e8tdnlmug5m2tamfn7q06sahstg39ys4c9f3340qqxrdu9k")
                .unwrap()
                .assume_checked();
        // let's calculate the required fee for a transaction with 2 funding utxos
        let args = EstimateEdictTxFeesArgs {
            script_type: ScriptType::P2WSH,
            number_of_inputs: 2 + 2,
            current_fee_rate: fee_rate,
            multisig_config: None,
            rune_change_address: address.clone(),
            destination_address: address.clone(),
            change_address: address.clone(),
            rune,
            rune_amount,
        };

        let fee = estimate_edict_transaction_fees(args);

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

        let greedy_funding_utxos = test_withdrawal()
            .get_greedy_funding_utxos(GetGreedyFundingUtxosArgs {
                rune_utxos_count: 2,
                funding_utxos,
                rune_change_address: address.clone(),
                destination_address: address.clone(),
                change_address: address.clone(),
                rune,
                rune_amount,
                fee_rate,
            })
            .unwrap();

        assert_eq!(greedy_funding_utxos.len(), 2);
        assert_eq!(greedy_funding_utxos[0].value, first_utxo_value);
        assert_eq!(greedy_funding_utxos[1].value, second_utxo_value);
    }

    #[test]
    fn test_return_none_in_case_not_enough_funds() {
        let rune = RuneId::new(219, 1).unwrap();
        let rune_amount = 9500;

        let fee_rate = FeeRate::from_sat_per_vb(1).unwrap();
        let address =
            Address::from_str("bc1pxwww0ct9ue7e8tdnlmug5m2tamfn7q06sahstg39ys4c9f3340qqxrdu9k")
                .unwrap()
                .assume_checked();

        // let's create funding utxos
        let funding_utxos = vec![
            Utxo {
                outpoint: Outpoint {
                    txid: vec![0; 32],
                    vout: 0,
                },
                value: 10,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![1; 32],
                    vout: 0,
                },
                value: 40,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![2; 32],
                    vout: 0,
                },
                value: 50,
                height: 0,
            },
            Utxo {
                outpoint: Outpoint {
                    txid: vec![3; 32],
                    vout: 0,
                },
                value: 12,
                height: 0,
            },
        ];

        let greedy_funding_utxos =
            test_withdrawal().get_greedy_funding_utxos(GetGreedyFundingUtxosArgs {
                rune_utxos_count: 2,
                funding_utxos,
                rune_change_address: address.clone(),
                destination_address: address.clone(),
                change_address: address.clone(),
                rune,
                rune_amount,
                fee_rate,
            });

        assert!(greedy_funding_utxos.is_none());
    }

    fn test_withdrawal() -> Withdrawal<FakeUtxoProvider> {
        let state = RuneState::default();
        let fake_utxo_provider = FakeUtxoProvider {
            fee_rate: FeeRate::from_sat_per_vb(1).unwrap(),
        };
        Withdrawal {
            state: Rc::new(RefCell::new(state)),
            utxo_provider: fake_utxo_provider,
            signer: BtcSignerType::Local(LocalSigner::new(PrivateKey::generate(
                bitcoin::Network::Regtest,
            ))),
            network: bitcoin::Network::Regtest,
        }
    }
}
