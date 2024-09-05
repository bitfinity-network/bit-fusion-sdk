use std::cell::RefCell;

use bridge_canister::bridge::{Operation, OperationAction, OperationContext};
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::order::{EncodedMintOrder, MintOrder};
use bridge_did::reason::BtcDeposit;
use bridge_utils::bft_events::{BurntEventData, MintedEventData, NotifyMinterEventData};
use candid::{CandidType, Decode, Principal};
use did::H160;
use ic_canister::virtual_canister_call;
use ic_exports::ic_kit::ic;
use ic_exports::icrc_types::icrc1::account::Account as IcrcAccount;
use ic_exports::icrc_types::icrc1::transfer::TransferError;
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::task::TaskOptions;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};

use crate::canister::{eth_address_to_subaccount, get_state};
use crate::ckbtc_client::{
    CkBtcLedgerClient, CkBtcMinterClient, RetrieveBtcError, RetrieveBtcOk, UpdateBalanceError,
    UtxoStatus,
};
use crate::interface::{BtcBridgeError, BtcWithdrawError};
use crate::state::State;

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum BtcBridgeOp {
    // Deposit operations:
    UpdateCkBtcBalance {
        eth_address: H160,
    },
    CollectCkBtcBalance {
        eth_address: H160,
    },
    TransferCkBtc {
        eth_address: H160,
        amount: u64,
    },
    CreateMintOrder {
        eth_address: H160,
        amount: u64,
    },
    MintErc20 {
        eth_address: H160,
        order: EncodedMintOrder,
    },
    ConfirmErc20Mint {
        order: EncodedMintOrder,
        eth_address: H160,
    },
    Erc20MintConfirmed(MintedEventData),

    // Withdraw operations:
    WithdrawBtc(BurntEventData),
    BtcWithdrawConfirmed {
        eth_address: H160,
    },
}

impl Operation for BtcBridgeOp {
    async fn progress(self, id: OperationId, ctx: RuntimeState<Self>) -> BftResult<Self> {
        match self {
            Self::UpdateCkBtcBalance { eth_address } => {
                log::debug!("UpdateCkBtcBalance: Eth address {eth_address}");
                let ckbtc_minter = get_state().borrow().ck_btc_minter();
                Self::update_ckbtc_balance(ckbtc_minter, &eth_address).await?;

                Ok(Self::CollectCkBtcBalance { eth_address })
            }
            Self::CollectCkBtcBalance { eth_address } => {
                log::debug!("CollectCkBtcBalance: Eth address {eth_address}");
                let ckbtc_ledger = get_state().borrow().ck_btc_ledger();
                let ckbtc_balance = Self::collect_ckbtc_balance(ckbtc_ledger, &eth_address).await?;

                Ok(Self::TransferCkBtc {
                    eth_address,
                    amount: ckbtc_balance,
                })
            }
            Self::TransferCkBtc {
                eth_address,
                amount,
            } => {
                log::debug!("TransferCkBtc: Eth address {eth_address}, amount {amount}");
                let (ckbtc_ledger, ckbtc_fee) = {
                    let state = get_state();
                    let state_ref = state.borrow();
                    (state_ref.ck_btc_ledger(), state_ref.ck_btc_ledger_fee())
                };

                let amount_minus_fee =
                    Self::transfer_ckbtc_to_bridge(ckbtc_ledger, &eth_address, amount, ckbtc_fee)
                        .await?;

                Ok(Self::CreateMintOrder {
                    eth_address,
                    amount: amount_minus_fee,
                })
            }
            Self::CreateMintOrder {
                eth_address,
                amount,
            } => {
                log::debug!("CreateMintOrder: Eth address {eth_address}, amount {amount}");
                let mint_order = Self::mint_erc20(ctx, &eth_address, id.nonce(), amount).await?;

                Ok(Self::MintErc20 {
                    order: mint_order,
                    eth_address,
                })
            }
            Self::MintErc20 { eth_address, order } => {
                log::debug!("MintErc20: Eth address {eth_address}");
                ctx.send_mint_transaction(&order).await?;

                Ok(Self::ConfirmErc20Mint { order, eth_address })
            }
            Self::ConfirmErc20Mint { .. } => Err(Error::FailedToProgress(
                "ConfirmErc20Mint task should progress only on the Minted EVM event".into(),
            )),
            Self::Erc20MintConfirmed { .. } => Err(Error::FailedToProgress(
                "ConfirmMint task should not progress".into(),
            )),
            Self::WithdrawBtc(event) => {
                log::debug!("WithdrawBtc: Eth address {}", event.sender);
                Self::withdraw_btc(&event).await?;

                Ok(Self::BtcWithdrawConfirmed {
                    eth_address: event.sender,
                })
            }
            Self::BtcWithdrawConfirmed { .. } => Err(Error::FailedToProgress(
                "ConfirmMint task should not progress".into(),
            )),
        }
    }

    fn is_complete(&self) -> bool {
        match self {
            Self::UpdateCkBtcBalance { .. } => false,
            Self::CollectCkBtcBalance { .. } => false,
            Self::TransferCkBtc { .. } => false,
            Self::CreateMintOrder { .. } => false,
            Self::MintErc20 { .. } => false,
            Self::ConfirmErc20Mint { .. } => false,
            Self::Erc20MintConfirmed { .. } => true,
            Self::WithdrawBtc { .. } => false,
            Self::BtcWithdrawConfirmed { .. } => true,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match self {
            Self::BtcWithdrawConfirmed { eth_address } => eth_address.clone(),
            Self::CollectCkBtcBalance { eth_address } => eth_address.clone(),
            Self::CreateMintOrder { eth_address, .. } => eth_address.clone(),
            Self::ConfirmErc20Mint { eth_address, .. } => eth_address.clone(),
            Self::Erc20MintConfirmed(MintedEventData { recipient, .. }) => recipient.clone(),
            Self::MintErc20 { eth_address, .. } => eth_address.clone(),
            Self::TransferCkBtc { eth_address, .. } => eth_address.clone(),
            Self::UpdateCkBtcBalance { eth_address } => eth_address.clone(),
            Self::WithdrawBtc(BurntEventData { sender, .. }) => sender.clone(),
        }
    }

    fn scheduling_options(&self) -> Option<TaskOptions> {
        match self {
            Self::UpdateCkBtcBalance { .. } => Some(
                TaskOptions::new()
                    .with_max_retries_policy(10)
                    .with_backoff_policy(BackoffPolicy::Fixed { secs: 5 }),
            ),
            Self::CollectCkBtcBalance { .. }
            | Self::MintErc20 { .. }
            | Self::CreateMintOrder { .. }
            | Self::TransferCkBtc { .. }
            | Self::WithdrawBtc(_) => Some(
                TaskOptions::new()
                    .with_max_retries_policy(3)
                    .with_backoff_policy(BackoffPolicy::Exponential {
                        secs: 2,
                        multiplier: 4,
                    }),
            ),
            Self::BtcWithdrawConfirmed { .. }
            | Self::ConfirmErc20Mint { .. }
            | Self::Erc20MintConfirmed(_) => None,
        }
    }

    async fn on_wrapped_token_minted(
        _ctx: RuntimeState<Self>,
        event: MintedEventData,
    ) -> Option<bridge_canister::bridge::OperationAction<Self>> {
        log::trace!("wrapped token minted");
        Some(OperationAction::Update {
            nonce: event.nonce,
            update_to: Self::Erc20MintConfirmed(event),
        })
    }

    async fn on_wrapped_token_burnt(
        _ctx: RuntimeState<Self>,
        event: BurntEventData,
    ) -> Option<bridge_canister::bridge::OperationAction<Self>> {
        log::trace!("wrapped token burnt");
        let memo = event.memo();
        Some(OperationAction::Create(Self::WithdrawBtc(event), memo))
    }

    async fn on_minter_notification(
        _ctx: RuntimeState<Self>,
        event: NotifyMinterEventData,
    ) -> Option<bridge_canister::bridge::OperationAction<Self>> {
        log::trace!(
            "got minter notification with type: {}",
            event.notification_type
        );
        let mut btc_deposit = match Decode!(&event.user_data, BtcDeposit) {
            Ok(icrc_burn) => icrc_burn,
            Err(e) => {
                log::warn!("failed to decode BftBridge notification into BtcDeposit: {e}");
                return None;
            }
        };

        // Approve tokens only if the burner owns recipient wallet.
        if event.tx_sender != btc_deposit.recipient {
            btc_deposit.approve_after_mint = None;
        }

        let memo = event.memo();

        Some(OperationAction::Create(
            BtcBridgeOp::UpdateCkBtcBalance {
                eth_address: btc_deposit.recipient,
            },
            memo,
        ))
    }
}

impl BtcBridgeOp {
    pub fn get_signed_mint_order(&self) -> Option<EncodedMintOrder> {
        match self {
            Self::ConfirmErc20Mint { order, .. } => Some(*order),
            Self::MintErc20 { order, .. } => Some(*order),
            _ => None,
        }
    }

    async fn update_ckbtc_balance(ckbtc_minter: Principal, eth_address: &H160) -> BftResult<()> {
        let self_id = ic::id();
        let subaccount = eth_address_to_subaccount(eth_address);

        match CkBtcMinterClient::from(ckbtc_minter)
            .update_balance(self_id, Some(subaccount))
            .await
            .unwrap_or_else(|err| {
                Err(UpdateBalanceError::TemporarilyUnavailable(format!(
                    "Failed to connect to ckBTC minter: {err:?}"
                )))
            }) {
            Ok(minted_utxos) => {
                if minted_utxos.is_empty() {
                    log::debug!("No new utxos found for {eth_address}");
                }
                for utxo in minted_utxos {
                    match utxo {
                        UtxoStatus::Minted { minted_amount, .. } => {
                            log::debug!("Minted {minted_amount} BTC for {eth_address}");
                        }
                        UtxoStatus::ValueTooSmall(value) => {
                            log::debug!("Value too small for {eth_address}: {value:?}");
                            return Err(BtcBridgeError::ValueTooSmall.into());
                        }
                        UtxoStatus::Tainted(utxo) => {
                            log::debug!("Tainted UTXO for {eth_address}: {utxo:?}");
                            return Err(BtcBridgeError::Tainted(utxo).into());
                        }
                        UtxoStatus::Checked(_) => {
                            return Err(BtcBridgeError::CkBtcMinter(
                                UpdateBalanceError::TemporarilyUnavailable(
                                    "KYT check passed, but mint failed. Try again later."
                                        .to_string(),
                                ),
                            )
                            .into())
                        }
                    }
                }

                Ok(())
            }
            Err(UpdateBalanceError::NoNewUtxos {
                current_confirmations: Some(current_confirmations),
                required_confirmations,
                ..
            }) => {
                log::debug!("No new utxos found for {eth_address} with {current_confirmations} confirmations, waiting for {required_confirmations} confirmations");
                Err(BtcBridgeError::WaitingForConfirmations.into())
            }
            Err(UpdateBalanceError::NoNewUtxos { .. }) => {
                log::debug!("No new utxos found for {eth_address}");
                Ok(())
            }
            Err(err) => Err(BtcBridgeError::CkBtcMinter(err).into()),
        }
    }

    /// Collect ckBTC balance for the given Ethereum address.
    async fn collect_ckbtc_balance(ckbtc_ledger: Principal, eth_address: &H160) -> BftResult<u64> {
        let icrc_account = IcrcAccount {
            owner: ic::id(),
            subaccount: Some(eth_address_to_subaccount(eth_address).0),
        };
        log::debug!("Collecting ckBTC balance for {eth_address}");
        // Get current ckBTC balance
        let ckbtc_amount = match CkBtcLedgerClient::from(ckbtc_ledger)
            .icrc1_balance_of(icrc_account)
            .await
        {
            Ok(amount) => amount.0.to_u64().unwrap_or_default(),
            Err((rejection_code, message)) => {
                log::error!("Failed to get current ckBTC balance: {rejection_code:?} {message}");
                return Err(BtcBridgeError::CkBtcLedgerBalance(rejection_code, message).into());
            }
        };

        log::debug!("Current ckBTC balance for {eth_address}: {ckbtc_amount}");

        if ckbtc_amount == 0 {
            return Err(BtcBridgeError::NothingToMint.into());
        }

        Ok(ckbtc_amount)
    }

    /// Transfer ckBTC from the deposit address to the BTC bridge.
    async fn transfer_ckbtc_to_bridge(
        ckbtc_ledger: Principal,
        eth_address: &H160,
        amount: u64,
        ckbtc_fee: u64,
    ) -> BftResult<u64> {
        let amount_minus_fee = amount
            .checked_sub(ckbtc_fee)
            .ok_or(BtcBridgeError::ValueTooSmall)?;

        if amount_minus_fee == 0 {
            return Err(BtcBridgeError::ValueTooSmall.into());
        }

        CkBtcLedgerClient::from(ckbtc_ledger)
            .icrc1_transfer(
                ic_exports::icrc_types::icrc1::account::Account {
                    owner: ic::id(),
                    subaccount: None,
                },
                amount_minus_fee.into(),
                ckbtc_fee.into(),
                Some(eth_address_to_subaccount(eth_address).0),
            )
            .await
            .unwrap_or_else(|e| {
                log::error!("icrc1_transfer failed: {e:?}");
                Err(TransferError::TemporarilyUnavailable)
            })
            .map_err(BtcBridgeError::CkBtcLedgerTransfer)?;

        Ok(amount_minus_fee)
    }

    /// Mint ERC20 tokens for the given Ethereum address.
    async fn mint_erc20(
        ctx: impl OperationContext,
        eth_address: &H160,
        nonce: u32,
        amount: u64,
    ) -> BftResult<EncodedMintOrder> {
        let state = get_state();

        log::debug!(
            "Minting {amount} BTC to {eth_address} with nonce {nonce} for token {}",
            state.borrow().token_address()
        );

        let mint_order =
            Self::prepare_mint_order(&ctx, &state, eth_address.clone(), amount, nonce).await?;

        Ok(mint_order)
    }

    /// Withdraw BTC from the bridge to the recipient address.
    async fn withdraw_btc(event: &BurntEventData) -> BftResult<()> {
        let state = get_state();

        let Ok(address) = String::from_utf8(event.recipient_id.clone()) else {
            return Err(BtcWithdrawError::InvalidRecipient(event.recipient_id.clone()).into());
        };

        let amount = event.amount.0.as_u64();
        log::trace!("Transferring {amount} ckBTC to {address}");

        let ck_btc_ledger = state.borrow().ck_btc_ledger();
        let ck_btc_minter = state.borrow().ck_btc_minter();
        let fee = state.borrow().ck_btc_ledger_fee();
        let account = Self::get_ckbtc_withdrawal_account(ck_btc_minter).await?;

        // ICRC1 takes fee on top of the amount
        let to_transfer = amount - fee;
        Self::transfer_ckbtc_to_minter(ck_btc_ledger, account, to_transfer, fee).await?;

        Self::request_btc_withdrawal(ck_btc_minter, address.to_string(), to_transfer).await?;

        Ok(())
    }

    /// Prepare mint order for the given Ethereum address.
    async fn prepare_mint_order(
        ctx: &impl OperationContext,
        state: &RefCell<State>,
        eth_address: H160,
        amount: u64,
        nonce: u32,
    ) -> BftResult<EncodedMintOrder> {
        log::trace!("preparing mint order");

        let (signer, mint_order) = {
            let state_ref = state.borrow();

            let sender_chain_id = state_ref.btc_chain_id();
            let sender = Id256::from_evm_address(&eth_address, sender_chain_id);
            let src_token = (&state_ref.ck_btc_ledger()).into();

            let recipient_chain_id = ctx.get_evm_params()?.chain_id;

            let mint_order = MintOrder {
                amount: amount.into(),
                sender,
                src_token,
                recipient: eth_address,
                dst_token: state_ref.token_address().clone(),
                nonce,
                sender_chain_id,
                recipient_chain_id,
                name: state_ref.token_name(),
                symbol: state_ref.token_symbol(),
                decimals: state_ref.decimals(),
                approve_spender: Default::default(),
                approve_amount: Default::default(),
                fee_payer: H160::zero(),
            };

            let signer = ctx.get_signer()?;

            (signer, mint_order)
        };

        let signed_mint_order = mint_order
            .encode_and_sign(&signer)
            .await
            .map_err(|err| BtcBridgeError::Sign(format!("{err:?}")))?;

        Ok(signed_mint_order)
    }

    /// Get the withdrawal account for the ckbtc minter.
    async fn get_ckbtc_withdrawal_account(ckbtc_minter: Principal) -> BftResult<IcrcAccount> {
        log::trace!("Requesting ckbtc withdrawal account");

        let account =
            virtual_canister_call!(ckbtc_minter, "get_withdrawal_account", (), IcrcAccount)
                .await
                .map_err(|err| {
                    log::error!("Failed to get withdrawal account: {err:?}");
                    BtcWithdrawError::from(RetrieveBtcError::TemporarilyUnavailable(
                        "get withdrawal account".to_string(),
                    ))
                })?;

        log::trace!("Got ckbtc withdrawal account: {account:?}");

        Ok(account)
    }

    /// Transfer ckBTC to the minter.
    async fn transfer_ckbtc_to_minter(
        ckbtc_ledger: Principal,
        to: IcrcAccount,
        amount: u64,
        fee: u64,
    ) -> BftResult<()> {
        log::trace!("Transferring {amount} ckbtc to {to:?} with fee {fee}");

        CkBtcLedgerClient::from(ckbtc_ledger)
            .icrc1_transfer(to, amount.into(), fee.into(), None)
            .await
            .map_err(|err| {
                log::error!("Failed to transfer ckBTC: {err:?}");
                BtcWithdrawError::from(RetrieveBtcError::TemporarilyUnavailable(
                    "ckBTC transfer failed".to_string(),
                ))
            })?
            .map_err(|err| {
                log::error!("Failed to transfer ckBTC: {err:?}");
                BtcWithdrawError::from(RetrieveBtcError::TemporarilyUnavailable(
                    "ckBTC transfer failed".to_string(),
                ))
            })?;

        log::trace!("Transferred {amount} ckbtc to {to:?} with fee {fee}");

        Ok(())
    }

    /// Request a BTC withdrawal from the minter.
    async fn request_btc_withdrawal(
        ckbtc_minter: Principal,
        address: String,
        amount: u64,
    ) -> BftResult<RetrieveBtcOk> {
        log::trace!("Requesting withdrawal of {amount} btc to {address}");

        let result = CkBtcMinterClient::from(ckbtc_minter)
            .retrieve_btc(address.clone(), amount)
            .await
            .map_err(|err| {
                log::error!("Failed to call retrieve_btc: {err:?}");
                BtcWithdrawError::from(RetrieveBtcError::TemporarilyUnavailable(
                    "retrieve_btc call failed".to_string(),
                ))
            })?
            .map_err(BtcWithdrawError::from)?;

        log::trace!("Withdrawal of {amount} btc to {address} requested");

        Ok(result)
    }
}
