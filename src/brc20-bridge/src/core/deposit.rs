use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bitcoin::{Address, Network};
use bridge_canister::bridge::OperationContext;
use bridge_canister::runtime::RuntimeState;
use bridge_did::id256::Id256;
use bridge_did::order::{MintOrder, SignedMintOrder};
use candid::{CandidType, Deserialize};
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{GetUtxosResponse, Utxo};
use rust_decimal::Decimal;
use serde::Serialize;

use super::index_provider::IcHttpClient;
use crate::brc20_info::{Brc20Info, Brc20Tick};
use crate::canister::{get_brc20_state, get_runtime_state};
use crate::core::index_provider::{Brc20IndexProvider, OrdIndexProvider};
use crate::core::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::interface::DepositError;
use crate::key::{BtcSignerType, KeyError};
use crate::ledger::UtxoKey;
use crate::ops::Brc20BridgeOp;
use crate::state::Brc20State;

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub enum DepositRequestStatus {
    /// Deposit request received but is not yet executed.
    Scheduled,
    /// No utxos containing brc20s found at the deposit address. Waiting for the utxos to be mined
    /// into a block.
    WaitingForInputs {
        requested_at: u64,
        current_ts: u64,
        next_retry_at: u64,
        waiting_until: u64,
        block_height: u32,
    },
    /// No utxos containing brc20s found at the deposit address. Deposit operation is cancelled.
    NothingToDeposit {
        block_height: u32,
    },
    /// Utxos with brc20s are found at the deposit address, but are not confirmed yet. Deposit will
    /// proceed after enough confirmations are received.
    WaitingForConfirmations {
        utxos: Vec<Utxo>,
        current_min_confirmations: u32,
        required_confirmations: u32,
        block_height: u32,
    },
    InvalidAmounts {
        requested_amounts: HashMap<Brc20Tick, u128>,
        actual_amounts: HashMap<Brc20Tick, u128>,
    },
    /// Mint orders are signed by the canister but are not sent to the BftBridge. The user may attempt
    /// to send them by themselves or wait for the canister to retry the operation.
    MintOrdersCreated {
        orders: Vec<MintOrderDetails>,
    },
    Minted {
        amounts: Vec<(Brc20Tick, u128, H256)>,
    },
    InternalError {
        details: String,
    },
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct MintOrderDetails {
    pub brc20_tick: Brc20Tick,
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
pub struct Brc20DepositPayload {
    pub dst_address: H160,
    pub erc20_address: H160,
    pub requested_amounts: Option<(Brc20Tick, u128)>,
    pub request_ts: u64,
    pub status: DepositRequestStatus,
}

impl Brc20DepositPayload {
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

pub(crate) struct Brc20Deposit<
    UTXO: UtxoProvider = IcUtxoProvider,
    INDEX: Brc20IndexProvider = OrdIndexProvider<IcHttpClient>,
> {
    brc20_state: Rc<RefCell<Brc20State>>,
    runtime_state: RuntimeState<Brc20BridgeOp>,
    network: Network,
    signer: BtcSignerType,
    utxo_provider: UTXO,
    index_provider: INDEX,
}

impl Brc20Deposit<IcUtxoProvider, OrdIndexProvider<IcHttpClient>> {
    pub fn new(
        state: Rc<RefCell<Brc20State>>,
        runtime_state: RuntimeState<Brc20BridgeOp>,
    ) -> Result<Self, DepositError> {
        let state_ref = state.borrow();

        let signing_strategy = get_runtime_state()
            .borrow()
            .config
            .borrow()
            .get_signing_strategy();

        let network = state_ref.network();
        let ic_network = state_ref.ic_btc_network();
        let indexer_urls = state_ref.indexer_urls();
        let signer = state_ref
            .btc_signer(&signing_strategy)
            .ok_or(DepositError::SignerNotInitialized)?;
        let consensus_threshold = state_ref.indexer_consensus_threshold();

        drop(state_ref);

        Ok(Self {
            brc20_state: state,
            runtime_state,
            network,
            signer,
            utxo_provider: IcUtxoProvider::new(ic_network),
            index_provider: OrdIndexProvider::new(
                IcHttpClient {},
                indexer_urls,
                consensus_threshold,
            ),
        })
    }

    pub fn get(runtime_state: RuntimeState<Brc20BridgeOp>) -> Result<Self, DepositError> {
        Self::new(get_brc20_state(), runtime_state)
    }
}

impl<UTXO: UtxoProvider, INDEX: Brc20IndexProvider> Brc20Deposit<UTXO, INDEX> {
    // Get input utxos
    pub async fn get_inputs(&self, dst_address: &H160) -> Result<Vec<Utxo>, DepositError> {
        let transit_address = self.get_transit_address(dst_address).await?;

        Ok(self.get_deposit_utxos(&transit_address).await?.utxos)
    }

    pub async fn get_brc20_balance(
        &self,
        dst_address: &H160,
        tick: &Brc20Tick,
    ) -> Result<u128, DepositError> {
        let transit_address = self.get_transit_address(dst_address).await?;
        let balances = self
            .index_provider
            .get_brc20_balances(&transit_address)
            .await?;

        let info = self.get_brc20_info(tick).await.ok_or_else(|| {
            DepositError::Unavailable(format!(
                "Brc20 information for {tick} is not available. Please try again later."
            ))
        })?;

        let amount = balances.get(tick).copied().unwrap_or_default();

        Self::get_integer_amount(amount, info.decimals)
    }

    /// Converts the amount to the integer representation of the token.
    fn get_integer_amount(amount: Decimal, decimals: u8) -> Result<u128, DepositError> {
        use rust_decimal::prelude::ToPrimitive;

        let factor = Decimal::new(10i64.pow(decimals as u32), 0);
        (amount * factor).trunc().to_u128().ok_or_else(|| {
            DepositError::AmountTooBig(format!(
                "Amount {amount} with {decimals} decimals is too large to be represented as u128."
            ))
        })
    }

    /// Check for confirmations
    pub async fn check_confirmations(
        &self,
        dst_address: &H160,
        utxos: &[Utxo],
    ) -> Result<(), DepositError> {
        let transit_address = self.get_transit_address(dst_address).await?;
        let mut utxo_response = self.get_deposit_utxos(&transit_address).await?;
        utxo_response.utxos.retain(|v| utxos.contains(v));

        self.validate_utxo_confirmations(&utxo_response)
            .map_err(|_| DepositError::UtxosNotConfirmed)
    }

    pub async fn get_deposit_utxos(
        &self,
        transit_address: &Address,
    ) -> Result<GetUtxosResponse, DepositError> {
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

    pub fn validate_utxo_confirmations(&self, utxo_info: &GetUtxosResponse) -> Result<(), u32> {
        let min_confirmations = self.brc20_state.borrow().min_confirmations();
        let utxo_min_confirmations = utxo_info
            .utxos
            .iter()
            .map(|utxo| utxo_info.tip_height - utxo.height + 1)
            .min()
            .unwrap_or_default();

        if min_confirmations > utxo_min_confirmations {
            Err(utxo_min_confirmations)
        } else {
            log::trace!(
                "Current utxo confirmations {} satisfies minimum {}. Proceeding.",
                utxo_min_confirmations,
                min_confirmations
            );
            Ok(())
        }
    }

    pub async fn get_brc20_info(&self, tick: &Brc20Tick) -> Option<Brc20Info> {
        match self.get_brc20_infos_from_state(tick) {
            Some(v) => Some(v),
            None => self.get_brc20_info_from_indexer(tick).await,
        }
    }

    fn get_brc20_infos_from_state(&self, tick: &Brc20Tick) -> Option<Brc20Info> {
        let state = self.brc20_state.borrow();
        state.brc20_info(tick)
    }

    async fn get_brc20_info_from_indexer(&self, tick: &Brc20Tick) -> Option<Brc20Info> {
        let brc20_list = self.index_provider.get_brc20_tokens().await.ok()?;
        let brc20s: HashMap<Brc20Tick, Brc20Info> = brc20_list
            .iter()
            .map(|(brc20_id, info)| (*brc20_id, *info))
            .collect();

        let res = match brc20s.get(tick) {
            Some(v) => Some(*v),
            None => {
                log::error!("Ord indexer didn't return a brc20 information for brc20 {tick} that was present in an UTXO");
                None
            }
        };

        self.brc20_state.borrow_mut().update_brc20_tokens(brc20s);

        res
    }

    pub fn create_unsigned_mint_order(
        &self,
        dst_address: &H160,
        token_address: &H160,
        amount: u128,
        brc20_info: Brc20Info,
        nonce: u32,
    ) -> MintOrder {
        let state_ref = self.brc20_state.borrow();

        let sender_chain_id = state_ref.btc_chain_id();
        let sender = Id256::from_evm_address(dst_address, sender_chain_id);
        let src_token = Id256::from_brc20_tick(brc20_info.tick.inner());

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
            name: brc20_info.tick.name_array(),
            symbol: brc20_info.tick.symbol_array(),
            decimals: brc20_info.decimals,
            approve_spender: Default::default(),
            approve_amount: Default::default(),
            fee_payer: H160::default(),
        }
    }

    pub async fn sign_mint_order(
        &self,
        mint_order: MintOrder,
    ) -> Result<SignedMintOrder, DepositError> {
        let signer = self.runtime_state.get_signer().map_err(|err| {
            DepositError::Unavailable(format!("cannot initialize signer: {err:?}"))
        })?;
        let signed_mint_order = mint_order
            .encode_and_sign(&signer)
            .await
            .map_err(|err| DepositError::Sign(format!("{err:?}")))?;

        Ok(signed_mint_order)
    }

    fn filter_out_used_utxos(
        &self,
        get_utxos_response: &mut GetUtxosResponse,
    ) -> Result<(), DepositError> {
        let state_ref = self.brc20_state.borrow();
        let ledger = state_ref.ledger();

        get_utxos_response
            .utxos
            .retain(|utxo| !ledger.unspent_utxos_contains(&UtxoKey::from(&utxo.outpoint)));

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_convert_amount_to_integer() {
        let amount = Decimal::new(1, 0);
        let decimals = 8;

        let res =
            Brc20Deposit::<IcUtxoProvider, OrdIndexProvider<IcHttpClient>>::get_integer_amount(
                amount, decimals,
            );

        assert_eq!(res.unwrap(), 100_000_000);
    }
}
