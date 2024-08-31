use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bitcoin::hashes::Hash;
use bitcoin::{Address, Network};
use bridge_canister::runtime::RuntimeState;
use bridge_did::id256::Id256;
use bridge_did::order::{MintOrder, SignedMintOrder};
use candid::{CandidType, Deserialize};
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{GetUtxosResponse, Utxo};
use serde::Serialize;

use super::index_provider::IcHttpClient;
use crate::canister::get_rune_state;
use crate::core::index_provider::{OrdIndexProvider, RuneIndexProvider};
use crate::core::rune_inputs::{GetInputsError, RuneInput, RuneInputProvider, RuneInputs};
use crate::core::utxo_handler::{RuneToWrap, UtxoHandler, UtxoHandlerError};
use crate::core::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::interface::DepositError;
use crate::key::{get_derivation_path_ic, BtcSignerType};
use crate::ledger::UnspentUtxoInfo;
use crate::ops::RuneBridgeOp;
use crate::rune_info::{RuneInfo, RuneName};
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
    /// Mint orders are signed by the canister but are not sent to the BftBridge. The user may attempt
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

pub(crate) struct RuneDeposit<
    UTXO: UtxoProvider = IcUtxoProvider,
    INDEX: RuneIndexProvider = OrdIndexProvider<IcHttpClient>,
> {
    rune_state: Rc<RefCell<RuneState>>,
    runtime_state: RuntimeState<RuneBridgeOp>,
    network: Network,
    signer: BtcSignerType,
    utxo_provider: UTXO,
    index_provider: INDEX,
}

impl RuneDeposit<IcUtxoProvider, OrdIndexProvider<IcHttpClient>> {
    pub fn new(state: Rc<RefCell<RuneState>>, runtime_state: RuntimeState<RuneBridgeOp>) -> Self {
        let state_ref = state.borrow();

        let network = state_ref.network();
        let ic_network = state_ref.ic_btc_network();
        let indexer_urls = state_ref.indexer_urls();
        let signer = state_ref.btc_signer();
        let consensus_threshold = state_ref.indexer_consensus_threshold();

        drop(state_ref);

        Self {
            rune_state: state,
            runtime_state,
            network,
            signer,
            utxo_provider: IcUtxoProvider::new(ic_network),
            index_provider: OrdIndexProvider::new(
                IcHttpClient {},
                indexer_urls,
                consensus_threshold,
            ),
        }
    }

    pub fn get(runtime_state: RuntimeState<RuneBridgeOp>) -> Self {
        Self::new(get_rune_state(), runtime_state)
    }
}

impl<UTXO: UtxoProvider, INDEX: RuneIndexProvider> RuneInputProvider for RuneDeposit<UTXO, INDEX> {
    async fn get_inputs(&self, dst_address: &H160) -> Result<RuneInputs, GetInputsError> {
        let transit_address = self.get_transit_address(dst_address).await;
        let utxos = self.get_deposit_utxos(&transit_address).await?.utxos;
        let mut inputs = RuneInputs::default();
        for utxo in utxos {
            let amounts = self.index_provider.get_rune_amounts(&utxo).await?;
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

impl<UTXO: UtxoProvider, INDEX: RuneIndexProvider> UtxoHandler for RuneDeposit<UTXO, INDEX> {
    async fn check_confirmations(
        &self,
        dst_address: &H160,
        utxo: &Utxo,
    ) -> Result<(), UtxoHandlerError> {
        let transit_address = self.get_transit_address(dst_address).await;
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
        let address = self.get_transit_address(dst_address).await;
        let derivation_path = get_derivation_path_ic(dst_address);

        {
            let mut state = self.rune_state.borrow_mut();
            let ledger = state.ledger_mut();
            let existing = ledger.load_unspent_utxos();

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

impl<UTXO: UtxoProvider, INDEX: RuneIndexProvider> RuneDeposit<UTXO, INDEX> {
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

        self.filter_out_used_utxos(&mut utxo_response);

        log::trace!(
            "Utxos at address {transit_address} after filtering out used utxos: {:?}",
            utxo_response.utxos
        );

        Ok(utxo_response)
    }

    async fn get_transit_address(&self, eth_address: &H160) -> Address {
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
        let rune_list = self.index_provider.get_rune_list().await.ok()?;
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
            .chain_id;

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

    fn filter_out_used_utxos(&self, get_utxos_response: &mut GetUtxosResponse) {
        let existing = self.rune_state.borrow().ledger().load_unspent_utxos();

        get_utxos_response.utxos.retain(|utxo| {
            !existing
                .values()
                .any(|v| Self::check_already_used_utxo(v, utxo))
        })
    }

    fn check_already_used_utxo(v: &UnspentUtxoInfo, utxo: &Utxo) -> bool {
        v.tx_input_info.outpoint.txid.as_byte_array()[..] == utxo.outpoint.txid
            && v.tx_input_info.outpoint.vout == utxo.outpoint.vout
    }

    pub async fn get_mint_amounts(
        &self,
        utxos: &[Utxo],
        requested_amounts: &Option<HashMap<RuneName, u128>>,
    ) -> Result<(Vec<(RuneInfo, u128)>, Vec<Utxo>), DepositError> {
        let mut rune_amounts = HashMap::new();
        let mut used_utxos = vec![];

        for utxo in utxos {
            log::info!("Get rune amounts for: {:?}", utxo);
            let tx_rune_amounts = match self.index_provider.get_rune_amounts(utxo).await {
                Ok(v) => v,
                Err(err) => {
                    log::error!("Failed to get rune amounts for utxo: {err:?}");
                    continue;
                }
            };

            if !tx_rune_amounts.is_empty() {
                used_utxos.push(utxo.clone());
                for (rune_name, amount) in tx_rune_amounts {
                    *rune_amounts.entry(rune_name).or_default() += amount;
                }
            }
        }

        if rune_amounts.is_empty() {
            return Err(DepositError::NoRunesToDeposit);
        }

        if let Some(requested) = requested_amounts {
            if rune_amounts != *requested {
                return Err(DepositError::InvalidAmounts {
                    requested: requested.clone(),
                    actual: rune_amounts,
                });
            }
        }

        let Some(rune_info_amounts) = self.get_rune_infos(&rune_amounts).await else {
            return Err(DepositError::Unavailable(
                "Ord indexer is in invalid state".to_string(),
            ));
        };

        Ok((rune_info_amounts, used_utxos))
    }
}
