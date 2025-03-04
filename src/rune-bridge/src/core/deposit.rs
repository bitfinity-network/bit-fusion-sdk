use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
use std::future::Future;
use std::rc::Rc;

use bitcoin::hashes::Hash;
use bitcoin::{Address, Network};
use bridge_canister::runtime::RuntimeState;
use bridge_did::id256::Id256;
use bridge_did::order::{MintOrder, SignedMintOrder};
use bridge_did::runes::{RuneInfo, RuneName, RuneToWrap};
use candid::{CandidType, Deserialize};
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{GetUtxosResponse, Utxo};
use serde::Serialize;

use super::index_provider::get_indexer;
use crate::canister::{get_rune_state, get_runtime_state};
use crate::core::index_provider::RuneIndexProvider;
use crate::core::rune_inputs::{GetInputsError, RuneInput, RuneInputProvider, RuneInputs};
use crate::core::utxo_handler::{UtxoHandler, UtxoHandlerError};
use crate::core::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::interface::DepositError;
use crate::key::{get_derivation_path_ic, BtcSignerType, KeyError};
use crate::ledger::UnspentUtxoInfo;
use crate::ops::RuneBridgeOpImpl;
use crate::state::RuneState;

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub enum DepositRequestStatus {
    /// Deposit request received but is not yet executed.
    Scheduled,
    /// No utxos containing runes found at the deposit address. Waiting for the utxos to be mined
    /// into a block.
    WaitingForInputs {
        requested_at: u64,
        current_ts: u64,
        next_retry_at: u64,
        waiting_until: u64,
        block_height: u32,
    },
    /// No utxos containing runes found at the deposit address. Deposit operation is cancelled.
    NothingToDeposit {
        block_height: u32,
    },
    /// Utxos with runes are found at the deposit address, but are not confirmed yet. Deposit will
    /// proceed after enough confirmations are received.
    WaitingForConfirmations {
        utxos: Vec<Utxo>,
        current_min_confirmations: u32,
        required_confirmations: u32,
        block_height: u32,
    },
    InvalidAmounts {
        requested_amounts: HashMap<RuneName, u128>,
        actual_amounts: HashMap<RuneName, u128>,
    },
    /// Mint orders are signed by the canister but are not sent to the Btfbridge. The user may attempt
    /// to send them by themselves or wait for the canister to retry the operation.
    MintOrdersCreated {
        orders: Vec<MintOrderDetails>,
    },
    Minted {
        amounts: Vec<(RuneName, u128, H256)>,
    },
    InternalError {
        details: String,
    },
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct MintOrderDetails {
    pub rune_name: RuneName,
    pub amount: u128,
    pub status: MintOrderStatus,
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub enum MintOrderStatus {
    Created {
        mint_order: SignedMintOrder,
        nonce: u32,
    },
    Sent {
        mint_order: SignedMintOrder,
        nonce: u32,
        tx_id: H256,
    },
    Completed {
        tx_id: H256,
    },
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct RuneDepositPayload {
    pub dst_address: H160,
    pub erc20_address: H160,
    pub requested_amounts: Option<HashMap<RuneName, u128>>,
    pub request_ts: u64,
    pub status: DepositRequestStatus,
}

impl RuneDepositPayload {
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            DepositRequestStatus::NothingToDeposit { .. }
                | DepositRequestStatus::InvalidAmounts { .. }
                | DepositRequestStatus::Minted { .. }
                | DepositRequestStatus::InternalError { .. }
        )
    }
}

pub(crate) struct RuneDeposit<UTXO: UtxoProvider = IcUtxoProvider> {
    rune_state: Rc<RefCell<RuneState>>,
    runtime_state: RuntimeState<RuneBridgeOpImpl>,
    network: Network,
    signer: BtcSignerType,
    utxo_provider: UTXO,
    indexers: Vec<Box<dyn RuneIndexProvider>>,
    indexer_consensus_threshold: u8,
}

impl RuneDeposit<IcUtxoProvider> {
    pub fn new(
        state: Rc<RefCell<RuneState>>,
        runtime_state: RuntimeState<RuneBridgeOpImpl>,
    ) -> Result<Self, DepositError> {
        let state_ref = state.borrow();

        let signer_strategy = get_runtime_state()
            .borrow()
            .config
            .borrow()
            .get_signing_strategy();

        let network = state_ref.network();
        let ic_network = state_ref.ic_btc_network();
        let cache_timeout = state_ref.utxo_cache_timeout();

        let indexer_configs = state_ref.indexers_config();
        let indexers = indexer_configs.into_iter().map(get_indexer).collect();

        let signer = state_ref
            .btc_signer(&signer_strategy)
            .ok_or(DepositError::SignerNotInitialized)?;
        let consensus_threshold = state_ref.indexer_consensus_threshold();

        drop(state_ref);

        Ok(Self {
            rune_state: state,
            runtime_state,
            network,
            signer,
            utxo_provider: IcUtxoProvider::new(ic_network, cache_timeout),
            indexers,
            indexer_consensus_threshold: consensus_threshold,
        })
    }

    pub fn get(runtime_state: RuntimeState<RuneBridgeOpImpl>) -> Result<Self, DepositError> {
        Self::new(get_rune_state(), runtime_state)
    }
}

impl<UTXO: UtxoProvider> RuneInputProvider for RuneDeposit<UTXO> {
    async fn get_inputs(&self, dst_address: &H160) -> Result<RuneInputs, GetInputsError> {
        let transit_address = self.get_transit_address(dst_address).await?;
        let utxos = self.get_deposit_utxos(&transit_address).await?.utxos;
        let mut inputs = RuneInputs::default();
        for utxo in utxos {
            let amounts = self
                .get_indexer_consensus(|indexer| {
                    let utxo = utxo.clone();
                    async move { indexer.get_rune_amounts(&utxo).await }
                })
                .await?;
            if !amounts.is_empty() {
                inputs.inputs.push(RuneInput {
                    utxo,
                    runes: amounts,
                });
            }
        }

        Ok(inputs)
    }

    async fn get_rune_infos(
        &self,
        rune_amounts: &HashMap<RuneName, u128>,
    ) -> Option<Vec<(RuneInfo, u128)>> {
        match self.get_rune_infos_from_state(rune_amounts) {
            Some(v) => Some(v),
            None => self.get_rune_infos_from_indexer(rune_amounts).await,
        }
    }
}

impl<UTXO: UtxoProvider> UtxoHandler for RuneDeposit<UTXO> {
    async fn check_confirmations(
        &self,
        dst_address: &H160,
        utxo: &Utxo,
    ) -> Result<(), UtxoHandlerError> {
        let transit_address = self.get_transit_address(dst_address).await?;
        let utxo_response = self
            .get_deposit_utxos(&transit_address)
            .await
            .map_err(|err| UtxoHandlerError::BtcAdapter(err.to_string()))?;
        let block_height = utxo_response.tip_height;
        // todo: height of the utxo may change. Replace the comparison here and wherever we compare
        // utxos and add unit test for this case
        let Some(found_utxo) = utxo_response.utxos.into_iter().find(|v| v == utxo) else {
            return Err(UtxoHandlerError::UtxoNotFound);
        };

        let min_confirmations = self.rune_state.borrow().min_confirmations();
        let current_confirmations = block_height.saturating_sub(found_utxo.height + 1);
        let is_confirmed = current_confirmations >= min_confirmations;
        if is_confirmed {
            Ok(())
        } else {
            Err(UtxoHandlerError::NotConfirmed {
                current_confirmations,
                required_confirmations: min_confirmations,
            })
        }
    }

    async fn deposit(
        &self,
        utxo: &Utxo,
        dst_address: &H160,
        utxo_runes: Vec<RuneToWrap>,
    ) -> Result<Vec<MintOrder>, UtxoHandlerError> {
        let address = self.get_transit_address(dst_address).await?;
        let derivation_path = get_derivation_path_ic(dst_address);

        {
            let mut state = self.rune_state.borrow_mut();
            let ledger = state.ledger_mut();
            let existing = ledger.load_unspent_utxos()?;

            if existing
                .values()
                .any(|v| Self::check_already_used_utxo(v, utxo))
            {
                return Err(UtxoHandlerError::UtxoAlreadyUsed);
            }

            let deposit_runes = utxo_runes.iter().map(|rune| rune.rune_info).collect();
            ledger.deposit(utxo.clone(), &address, derivation_path, deposit_runes);
        }

        let mut mint_orders = vec![];
        for to_wrap in utxo_runes {
            let mint_order = self.create_unsigned_mint_order(
                dst_address,
                &to_wrap.wrapped_address,
                to_wrap.amount,
                to_wrap.rune_info,
                0,
            );
            mint_orders.push(mint_order);
        }

        Ok(mint_orders)
    }
}

impl<UTXO: UtxoProvider> RuneDeposit<UTXO> {
    pub async fn get_deposit_utxos(
        &self,
        transit_address: &Address,
    ) -> Result<GetUtxosResponse, GetInputsError> {
        let mut utxo_response = self.utxo_provider.get_utxos(transit_address).await?;

        log::trace!(
            "Found {} utxos at address {transit_address}: {:?}.",
            utxo_response.utxos.len(),
            utxo_response.utxos
        );

        self.filter_out_used_utxos(&mut utxo_response)?;

        log::trace!(
            "Utxos at address {transit_address} after filtering out used utxos: {:?}",
            utxo_response.utxos
        );

        Ok(utxo_response)
    }

    async fn get_transit_address(&self, eth_address: &H160) -> Result<Address, KeyError> {
        self.signer
            .get_transit_address(eth_address, self.network)
            .await
    }

    fn get_rune_infos_from_state(
        &self,
        rune_amounts: &HashMap<RuneName, u128>,
    ) -> Option<Vec<(RuneInfo, u128)>> {
        let state = self.rune_state.borrow();
        let runes = state.runes();
        let mut infos = vec![];
        for (rune_name, amount) in rune_amounts {
            infos.push((*runes.get(rune_name)?, *amount));
        }

        Some(infos)
    }

    async fn get_rune_infos_from_indexer(
        &self,
        rune_amounts: &HashMap<RuneName, u128>,
    ) -> Option<Vec<(RuneInfo, u128)>> {
        let rune_list = self
            .get_indexer_consensus(|indexer| async move { indexer.get_rune_list().await })
            .await
            .ok()?;
        let runes: HashMap<RuneName, RuneInfo> = rune_list
            .iter()
            .map(|(rune_id, spaced_rune, decimals)| {
                (
                    spaced_rune.rune.into(),
                    RuneInfo {
                        name: spaced_rune.rune.into(),
                        decimals: *decimals,
                        block: rune_id.block,
                        tx: rune_id.tx,
                    },
                )
            })
            .collect();
        let mut infos = vec![];
        for (rune_name, amount) in rune_amounts {
            match runes.get(rune_name) {
                Some(v) => infos.push((*v, *amount)),
                None => {
                    log::error!("Ord indexer didn't return a rune information for rune {rune_name} that was present in an UTXO");
                    return None;
                }
            }
        }

        self.rune_state.borrow_mut().update_rune_list(runes);

        Some(infos)
    }

    async fn get_indexer_consensus<
        'a,
        T: Debug + PartialEq,
        E: Error,
        F: Future<Output = Result<T, E>> + 'a,
    >(
        &'a self,
        request: impl Fn(&'a dyn RuneIndexProvider) -> F,
    ) -> Result<T, GetInputsError> {
        let consensus_threshold = self.indexer_consensus_threshold;
        let mut first_result = None;
        let mut requested = 0;
        let mut received_responses = 0;
        for indexer in &self.indexers {
            let result = request(&**indexer).await;
            requested += 1;
            if let Ok(response) = result {
                match first_result {
                    None => first_result = Some(response),
                    Some(ref r) => {
                        received_responses += 1;
                        if *r != response {
                            return Err(GetInputsError::IndexersDisagree {
                                first_response: format!("{r:?}"),
                                another_response: format!("{response:?}"),
                            });
                        }
                    }
                }
            } else {
                log::warn!("Indexer responded with {result:?}");
                if (self.indexers.len() as u8).saturating_sub(requested)
                    < consensus_threshold.saturating_sub(received_responses)
                {
                    return Err(GetInputsError::InsufficientConsensus {
                        received_responses: received_responses as usize,
                        required_responses: consensus_threshold,
                        checked_indexers: requested as usize,
                    });
                }
            }
        }

        match first_result {
            Some(v) => Ok(v),
            None => {
                // This means that `consensus_threshold` is 0 and none of the indexers are online
                // or that there are not indexers configured
                Err(GetInputsError::InsufficientConsensus {
                    received_responses: 0,
                    required_responses: consensus_threshold,
                    checked_indexers: self.indexers.len(),
                })
            }
        }
    }

    pub fn create_unsigned_mint_order(
        &self,
        dst_address: &H160,
        token_address: &H160,
        amount: u128,
        rune_info: RuneInfo,
        nonce: u32,
    ) -> MintOrder {
        let state_ref = self.rune_state.borrow();

        let sender_chain_id = state_ref.btc_chain_id();
        let sender = Id256::from_evm_address(dst_address, sender_chain_id);
        let src_token = Id256::from(rune_info.id());

        let recipient_chain_id = self
            .runtime_state
            .borrow()
            .config
            .borrow()
            .get_evm_params()
            .unwrap()
            .chain_id as u32;

        MintOrder {
            amount: amount.into(),
            sender,
            src_token,
            recipient: dst_address.clone(),
            dst_token: token_address.clone(),
            nonce,
            sender_chain_id,
            recipient_chain_id,
            name: rune_info.name_array(),
            symbol: rune_info.symbol_array(),
            decimals: rune_info.decimals(),
            approve_spender: Default::default(),
            approve_amount: Default::default(),
            fee_payer: H160::default(),
        }
    }

    fn filter_out_used_utxos(
        &self,
        get_utxos_response: &mut GetUtxosResponse,
    ) -> Result<(), KeyError> {
        let existing = self.rune_state.borrow().ledger().load_unspent_utxos()?;

        get_utxos_response.utxos.retain(|utxo| {
            !existing
                .values()
                .any(|v| Self::check_already_used_utxo(v, utxo))
        });

        Ok(())
    }

    fn check_already_used_utxo(v: &UnspentUtxoInfo, utxo: &Utxo) -> bool {
        v.tx_input_info.outpoint.txid.as_byte_array()[..] == utxo.outpoint.txid
            && v.tx_input_info.outpoint.vout == utxo.outpoint.vout
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::atomic::{AtomicU8, Ordering};

    use async_trait::async_trait;
    use bitcoin::secp256k1::Secp256k1;
    use bitcoin::{FeeRate, PrivateKey, Transaction};
    use bridge_canister::memory::{memory_by_id, StableMemory};
    use bridge_canister::operation_store::OperationsMemory;
    use bridge_canister::runtime::state::config::ConfigStorage;
    use bridge_canister::runtime::state::{SharedConfig, State};
    use ic_stable_structures::MemoryId;
    use ord_rs::wallet::LocalSigner;
    use ordinals::{RuneId, SpacedRune};

    use super::*;
    use crate::interface::WithdrawError;

    fn op_memory() -> OperationsMemory<StableMemory> {
        OperationsMemory {
            id_counter: memory_by_id(MemoryId::new(1)),
            incomplete_operations: memory_by_id(MemoryId::new(2)),
            operations_log: memory_by_id(MemoryId::new(3)),
            operations_map: memory_by_id(MemoryId::new(4)),
            memo_operations_map: memory_by_id(MemoryId::new(5)),
        }
    }

    fn config() -> SharedConfig {
        Rc::new(RefCell::new(ConfigStorage::default(memory_by_id(
            MemoryId::new(5),
        ))))
    }

    fn test_state() -> RuntimeState<RuneBridgeOpImpl> {
        Rc::new(RefCell::new(State::default(op_memory(), config())))
    }

    fn signer() -> BtcSignerType {
        let s = Secp256k1::new();
        let keypair = s.generate_keypair(&mut rand::thread_rng());
        let signer = LocalSigner::new(PrivateKey::new(keypair.0, Network::Bitcoin));

        BtcSignerType::Local(signer)
    }

    struct TestUtxoProvider;
    impl UtxoProvider for TestUtxoProvider {
        async fn get_utxos(&self, _address: &Address) -> Result<GetUtxosResponse, GetInputsError> {
            unimplemented!()
        }

        async fn get_fee_rate(&self) -> Result<FeeRate, WithdrawError> {
            unimplemented!()
        }

        async fn send_tx(&self, _transaction: &Transaction) -> Result<(), WithdrawError> {
            unimplemented!()
        }
    }

    struct TestIndexerProvider {
        value: u8,
    }

    #[async_trait(?Send)]
    impl RuneIndexProvider for TestIndexerProvider {
        async fn get_rune_amounts(
            &self,
            _utxo: &Utxo,
        ) -> Result<HashMap<RuneName, u128>, GetInputsError> {
            Ok([(RuneName::from_str("A").unwrap(), self.value as u128)].into())
        }

        async fn get_rune_list(&self) -> Result<Vec<(RuneId, SpacedRune, u8)>, GetInputsError> {
            unimplemented!()
        }
    }

    fn test_ctx(indexers_count: u8, consensus_threshold: u8) -> RuneDeposit<TestUtxoProvider> {
        test_ctx_with_responses(
            &(0..indexers_count).collect::<Vec<u8>>(),
            consensus_threshold,
        )
    }

    fn test_ctx_with_responses(
        responses: &[u8],
        consensus_threshold: u8,
    ) -> RuneDeposit<TestUtxoProvider> {
        let mut indexers: Vec<Box<dyn RuneIndexProvider>> = vec![];
        for v in responses {
            indexers.push(Box::new(TestIndexerProvider { value: *v }));
        }

        RuneDeposit {
            rune_state: Rc::new(RefCell::new(Default::default())),
            runtime_state: test_state(),
            network: Network::Bitcoin,
            signer: signer(),
            utxo_provider: TestUtxoProvider,
            indexers,
            indexer_consensus_threshold: consensus_threshold,
        }
    }

    #[tokio::test]
    async fn get_consensus_single_indexer() {
        let deposit = test_ctx(1, 1);
        let result = deposit
            .get_indexer_consensus(|_| async { Ok::<_, GetInputsError>(true) })
            .await;
        assert_eq!(result, Ok(true));
    }

    #[tokio::test]
    async fn get_consensus_single_indexer_offline() {
        let deposit = test_ctx(1, 1);
        let result = deposit
            .get_indexer_consensus(|_| async {
                Err::<bool, _>(GetInputsError::IndexerError("offline".to_string()))
            })
            .await;
        assert_eq!(
            result,
            Err(GetInputsError::InsufficientConsensus {
                received_responses: 0,
                required_responses: 1,
                checked_indexers: 1,
            })
        );
    }

    #[tokio::test]
    async fn get_consensus_multiple_indexers_agree() {
        let deposit = test_ctx(5, 5);
        let result = deposit
            .get_indexer_consensus(|_| async { Ok::<_, GetInputsError>(true) })
            .await;
        assert_eq!(result, Ok(true));
    }

    #[tokio::test]
    async fn get_consensus_multiple_indexers_disagree() {
        let deposit = test_ctx(5, 5);
        let result = deposit
            .get_indexer_consensus(|indexer| async {
                indexer.get_rune_amounts(&Utxo::default()).await
            })
            .await;
        assert!(matches!(
            result,
            Err(GetInputsError::IndexersDisagree { .. })
        ));
    }

    #[tokio::test]
    async fn get_consensus_multiple_indexers_one_disagree() {
        let deposit = test_ctx_with_responses(&[1, 2, 1], 3);
        let result = deposit
            .get_indexer_consensus(|indexer| async {
                indexer.get_rune_amounts(&Utxo::default()).await
            })
            .await;
        assert!(matches!(
            result,
            Err(GetInputsError::IndexersDisagree { .. })
        ));
    }

    #[tokio::test]
    async fn get_consensus_multiple_indexers_disagree_under_threshold() {
        let deposit = test_ctx_with_responses(&[1, 2, 1], 2);
        let result = deposit
            .get_indexer_consensus(|indexer| async {
                indexer.get_rune_amounts(&Utxo::default()).await
            })
            .await;
        assert!(matches!(
            result,
            Err(GetInputsError::IndexersDisagree { .. })
        ));
    }

    #[tokio::test]
    async fn get_consensus_no_indexers() {
        let deposit = test_ctx(0, 0);
        let result = deposit
            .get_indexer_consensus(|_| async { Ok::<_, GetInputsError>(true) })
            .await;
        assert_eq!(
            result,
            Err(GetInputsError::InsufficientConsensus {
                received_responses: 0,
                required_responses: 0,
                checked_indexers: 0,
            })
        )
    }

    #[tokio::test]
    async fn get_consensus_threshold_larger_than_no_of_indexers() {
        let deposit = test_ctx(3, 5);
        let result = deposit
            .get_indexer_consensus(|_| async { Ok::<_, GetInputsError>(true) })
            .await;
        assert_eq!(result, Ok(true))
    }

    #[tokio::test]
    async fn get_consensus_all_indexers_are_requested() {
        let deposit = test_ctx_with_responses(&[1, 1, 1, 1, 2], 4);
        let result = deposit
            .get_indexer_consensus(|indexer| async {
                indexer.get_rune_amounts(&Utxo::default()).await
            })
            .await;
        assert!(matches!(
            result,
            Err(GetInputsError::IndexersDisagree { .. })
        ))
    }

    #[tokio::test]
    async fn get_consensus_threshold_achieved() {
        const THRESHOLD: u8 = 3;
        let deposit = test_ctx(5, THRESHOLD);
        let no_of_requests = AtomicU8::new(0);
        let result = deposit
            .get_indexer_consensus(|_| async {
                let count = no_of_requests.fetch_add(1, Ordering::Relaxed);
                if count > THRESHOLD {
                    Err(GetInputsError::IndexerError("inevitable".to_string()))
                } else {
                    Ok(true)
                }
            })
            .await;
        assert_eq!(result, Ok(true))
    }

    #[tokio::test]
    async fn get_consensus_threshold_not_achieved() {
        const THRESHOLD: u8 = 3;
        let deposit = test_ctx(5, THRESHOLD);
        let no_of_requests = AtomicU8::new(0);
        let result = deposit
            .get_indexer_consensus(|_| async {
                let count = no_of_requests.fetch_add(1, Ordering::Relaxed);
                if count > THRESHOLD - 1 {
                    Err(GetInputsError::IndexerError("inevitable".to_string()))
                } else {
                    Ok(true)
                }
            })
            .await;
        assert_eq!(
            result,
            Err(GetInputsError::InsufficientConsensus {
                received_responses: 2,
                required_responses: THRESHOLD,
                checked_indexers: 5,
            })
        );
    }
}
