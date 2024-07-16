use bridge_canister::{
    bridge::{EventHandler, Operation, OperationAction, OperationContext},
    runtime::state::SharedConfig,
};
use bridge_did::{error::BftResult, op_id::OperationId};
use bridge_utils::bft_events::{BurntEventData, MintedEventData, NotifyMinterEventData};
use serde::{Deserialize, Serialize};

pub struct IcrcEventHandler {}

impl EventHandler for IcrcEventHandler {
    type Stage = IcrcBridgeOp;

    async fn on_wrapped_token_minted(
        &self,
        event: MintedEventData,
    ) -> Option<OperationAction<Self::Stage>> {
        todo!()
    }

    async fn on_wrapped_token_burnt(
        &self,
        event: BurntEventData,
    ) -> Option<OperationAction<Self::Stage>> {
        todo!()
    }

    async fn on_minter_notification(
        &self,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self::Stage>> {
        todo!()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum IcrcBridgeOp {
    BurnIcrc2Tokens(OperationId),
    PrepareMintOrder(OperationId),
    RemoveMintOrder(MintedEventData),
    SendMintTransaction(OperationId),
    MintIcrc2Tokens(OperationId),
}

impl Operation for IcrcBridgeOp {
    async fn progress(self, ctx: impl OperationContext) -> BftResult<Self> {
        let config = ConfigStorage::get();
        match self {
            IcrcBridgeOp::CollectEvmEvents => Box::pin(Self::collect_evm_events(core, scheduler)),
            IcrcBridgeOp::BurnIcrc2Tokens(operation_id) => {
                Box::pin(Self::burn_icrc2_tokens(scheduler, *operation_id))
            }
            IcrcBridgeOp::PrepareMintOrder(operation_id) => {
                Box::pin(Self::prepare_mint_order(core, scheduler, *operation_id))
            }
            IcrcBridgeOp::RemoveMintOrder(data) => {
                let data = data.clone();
                Box::pin(async move { Self::remove_mint_order(data) })
            }
            IcrcBridgeOp::SendMintTransaction(operation_id) => {
                Box::pin(Self::send_mint_transaction(core, *operation_id))
            }
            IcrcBridgeOp::MintIcrc2Tokens(operation_id) => {
                Box::pin(Self::mint_icrc2(*operation_id, scheduler))
            }
        }
    }

    fn is_complete(&self) -> bool {
        todo!()
    }
}

impl IcrcBridgeOp {
    pub async fn init_evm_info(config: SharedConfig) -> Result<EvmParams, SchedulerError> {
        log::trace!("evm info initialization started");

        let client = config.borrow().get_evm_link().get_json_rpc_client();
        let address = {
            let signer = config.borrow().get_signer().into_scheduler_result()?;
            signer.get_address().await.into_scheduler_result()?
        };

        let evm_params = EvmParams::query(client, address)
            .await
            .into_scheduler_result()?;

        config
            .borrow_mut()
            .update_evm_params(|p| *p = evm_params.clone());

        log::trace!("evm parameters initialized");

        Ok(evm_params)
    }

    async fn collect_evm_events(
        config: SharedConfig,
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Result<(), SchedulerError> {
        // We need to get a local variable for the lock here to make sure it's not dropped immediately.
        let Some(lock) = CollectLogsLock::take() else {
            log::trace!("Another collect evm events task is in progress. Skipping.");
            return Ok(());
        };

        log::trace!("collecting evm events");

        let client = config.borrow().get_evm_link().get_json_rpc_client();
        let evm_params = config.borrow().get_evm_params();
        let params = match evm_params {
            Ok(params) => params,
            Err(_) => {
                log::info!("No evm parameters set, trying to initialize evm info");
                Self::init_evm_info(config.clone()).await?
            }
        };

        let Some(bridge_contract) = config.borrow().get_bft_bridge_contract() else {
            log::warn!("no bft bridge contract set, unable to collect events");
            return Err(SchedulerError::TaskExecutionFailed(
                "no bft bridge contract set".into(),
            ));
        };

        let last_chain_block = client.get_block_number().await.into_scheduler_result()?;
        let last_request_block = last_chain_block.min(params.next_block + MAX_LOG_REQUEST_COUNT);

        let logs = BridgeEvent::collect_logs(
            &client,
            params.next_block,
            last_request_block,
            bridge_contract.0,
        )
        .await
        .into_scheduler_result()?;

        log::debug!("Got evm logs between blocks {} and {last_request_block} (last chain block is {last_chain_block}: {logs:?}", params.next_block);

        config
            .borrow_mut()
            .update_evm_params(|params| params.next_block = last_request_block + 1);

        log::trace!("appending logs to tasks: {logs:?}");

        scheduler.append_tasks(logs.into_iter().filter_map(Self::task_by_log).collect());

        // Update EVM params
        Self::update_evm_params(config.clone()).await?;

        // This line is not necessary as the lock variable will not be dropped anyway only at the
        // end of the function or on any early return from the function. But I couldn't find
        // explicit guarantees by the language for this behaviour, so let's make sure it's not dropped
        // until now.
        drop(lock);

        Ok(())
    }

    pub async fn burn_icrc2_tokens(
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
        operation_id: OperationId,
    ) -> Result<(), SchedulerError> {
        log::trace!("Operation {operation_id}: String burn_icrc2_tokens");

        let mut operation_store = get_operations_store();
        let operation_state = operation_store.get(operation_id);
        let Some(OperationState::Deposit(DepositOperationState::Scheduled(reason))) =
            operation_state
        else {
            log::error!(
                "Operation {operation_id}: deposit request was in incorrect state: {:?}",
                operation_state
            );
            return Ok(());
        };

        log::trace!("Operation {operation_id}: got operation data from the store: {reason:?}");

        let caller_account = Account {
            owner: reason.sender,
            subaccount: reason.from_subaccount,
        };

        let token_info = icrc1::query_token_info_or_read_from_cache(reason.icrc2_token_principal)
            .await
            .ok_or(Error::Custom {
                code: 0,
                msg: "failed to get token info".into(),
            })
            .into_scheduler_result()?;

        log::trace!("Operation {operation_id}: got token info: {token_info:?}");

        let name = order::fit_str_to_array(&token_info.name);
        let symbol = order::fit_str_to_array(&token_info.symbol);

        let spender_subaccount = address_to_icrc_subaccount(&reason.recipient_address.0);
        icrc2::burn(
            reason.icrc2_token_principal,
            caller_account,
            Some(spender_subaccount),
            (&reason.amount).into(),
            true,
        )
        .await
        .into_scheduler_result()?;

        log::trace!("Operation {operation_id}: transferred icrc tokens to the bridge account");

        let nonce = operation_id.nonce();
        let burn_data = BurntIcrc2Data {
            sender: reason.sender,
            amount: reason.amount,
            operation_id: nonce,
            name,
            symbol,
            decimals: token_info.decimals,
            src_token: reason.icrc2_token_principal,
            recipient_address: reason.recipient_address,
            erc20_token_address: reason.erc20_token_address,
            fee_payer: reason.fee_payer,
            approve_after_mint: reason.approve_after_mint,
        };

        log::trace!(
            "Operation {operation_id}: storing new operation status Icrc2Burned({burn_data:?})"
        );

        operation_store.update(
            operation_id,
            OperationState::Deposit(DepositOperationState::Icrc2Burned(burn_data)),
        );

        let options = TaskOptions::default()
            .with_backoff_policy(BackoffPolicy::Fixed { secs: 4 })
            .with_retry_policy(ic_task_scheduler::retry::RetryPolicy::Infinite);

        let task_id = scheduler
            .append_task(IcrcBridgeOp::PrepareMintOrder(operation_id).into_scheduled(options));

        log::trace!(
            "Operation {operation_id}: PrepareMintOrder task {task_id} is added to the scheduler"
        );

        Ok(())
    }

    async fn prepare_mint_order(
        config: SharedConfig,
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
        operation_id: OperationId,
    ) -> Result<(), SchedulerError> {
        let mut operation_store = get_operations_store();
        let operation_state = operation_store.get(operation_id);
        let (burnt_data, is_deposit) = match operation_state {
            Some(OperationState::Deposit(DepositOperationState::Icrc2Burned(burnt_data))) => {
                (burnt_data, true)
            }
            Some(OperationState::Withdrawal(WithdrawalOperationState::RefundScheduled(
                burnt_data,
            ))) => (burnt_data, false),
            _ => {
                log::error!(
                    "deposit request was in incorrect state: {:?}",
                    operation_state
                );
                return Ok(());
            }
        };

        log::trace!("preparing mint order. Is deposit: {is_deposit}: {burnt_data:?}");

        let Ok(evm_params) = config.borrow().get_evm_params() else {
            log::warn!("no evm parameters set, unable to prepare mint order");
            return Err(SchedulerError::TaskExecutionFailed(
                "no evm parameters set".into(),
            ));
        };

        let sender_chain_id = IC_CHAIN_ID;
        let recipient_chain_id = evm_params.chain_id;

        let sender = Id256::from(&burnt_data.sender);
        let src_token = Id256::from(&burnt_data.src_token);

        let nonce = burnt_data.operation_id;

        // If there is no fee payer, user should send mint tx by himself.
        let fee_payer = burnt_data.fee_payer.unwrap_or_default();
        let should_send_mint_tx = fee_payer != H160::zero();

        let (approve_spender, approve_amount) = burnt_data
            .approve_after_mint
            .map(|approve| (approve.approve_spender, approve.approve_amount))
            .unwrap_or_default();

        let mint_order = MintOrder {
            amount: burnt_data.amount,
            sender,
            src_token,
            recipient: burnt_data.recipient_address,
            dst_token: burnt_data.erc20_token_address,
            nonce,
            sender_chain_id,
            recipient_chain_id,
            name: burnt_data.name,
            symbol: burnt_data.symbol,
            decimals: burnt_data.decimals,
            approve_spender,
            approve_amount,
            fee_payer,
        };

        log::debug!("PREPARED MINT ORDER: {:?}", mint_order);

        let signer = config.borrow().get_signer().into_scheduler_result()?;
        let signed_mint_order = mint_order
            .encode_and_sign(&signer)
            .await
            .into_scheduler_result()?;

        if is_deposit {
            operation_store.update(
                operation_id,
                OperationState::Deposit(DepositOperationState::MintOrderSigned {
                    token_id: src_token,
                    amount: mint_order.amount,
                    signed_mint_order: Box::new(signed_mint_order),
                }),
            );
        } else {
            operation_store.update(
                operation_id,
                OperationState::Withdrawal(WithdrawalOperationState::RefundMintOrderSigned {
                    token_id: src_token,
                    amount: mint_order.amount,
                    signed_mint_order: Box::new(signed_mint_order),
                }),
            );
        }

        if should_send_mint_tx {
            // Update EVM params before sending the transaction.
            Self::update_evm_params(config.clone()).await?;

            let options = TaskOptions::default();
            scheduler.append_task(
                IcrcBridgeOp::SendMintTransaction(operation_id).into_scheduled(options),
            );
        }

        log::trace!("Mint order added");

        Ok(())
    }

    fn task_by_log(log: Log) -> Option<ScheduledTask<IcrcBridgeOp>> {
        log::trace!("creating task from the log: {log:?}");

        const TASK_RETRY_DELAY_SECS: u32 = 5;

        let options = TaskOptions::default()
            .with_backoff_policy(BackoffPolicy::Fixed {
                secs: TASK_RETRY_DELAY_SECS,
            })
            .with_max_retries_policy(u32::MAX);

        match BridgeEvent::from_log(log).into_scheduler_result() {
            Ok(BridgeEvent::Burnt(burnt)) => {
                log::debug!("Adding MintIcrc2 task");
                let operation_id = get_operations_store()
                    .new_operation(burnt.sender.clone(), OperationState::new_withdrawal(burnt));
                let mint_icrc2_task = IcrcBridgeOp::MintIcrc2Tokens(operation_id);
                return Some(mint_icrc2_task.into_scheduled(options));
            }
            Ok(BridgeEvent::Minted(minted)) => {
                log::debug!("Adding RemoveMintOrder task");
                let remove_mint_order_task = IcrcBridgeOp::RemoveMintOrder(minted);
                return Some(remove_mint_order_task.into_scheduled(options));
            }
            Ok(BridgeEvent::Notify(notification)) => {
                log::debug!("Adding BurnIcrc2 task");
                let mut icrc_burn = match Decode!(&notification.user_data, Icrc2Burn) {
                    Ok(icrc_burn) => icrc_burn,
                    Err(e) => {
                        log::warn!("failed to decode BftBridge notification into Icrc2Burn: {e}");
                        return None;
                    }
                };

                // Approve tokens only if the burner owns recipient wallet.
                if notification.tx_sender != icrc_burn.recipient_address {
                    icrc_burn.approve_after_mint = None;
                }

                let operation_id = get_operations_store().new_operation(
                    icrc_burn.recipient_address.clone(),
                    OperationState::new_deposit(icrc_burn),
                );
                let icrc_burn_task = IcrcBridgeOp::BurnIcrc2Tokens(operation_id);
                return Some(icrc_burn_task.into_scheduled(options));
            }
            Err(e) => log::warn!("collected log is incompatible with expected events: {e}"),
        }

        None
    }

    fn remove_mint_order(minted_event: MintedEventData) -> Result<(), SchedulerError> {
        log::trace!("mint order removing");

        let src_token = Id256::from_slice(&minted_event.from_token).ok_or_else(|| {
            log::error!("failed to decode token id256 from minted event",);
            SchedulerError::TaskExecutionFailed(
                "failed to decode token id256 from minted event".into(),
            )
        })?;

        let mut operation_store = get_operations_store();
        let nonce = minted_event.nonce;
        let Some((operation_id, operation_state)) = operation_store
            .get_for_address(&minted_event.recipient, None, None)
            .into_iter()
            .find(|(operation_id, _)| operation_id.nonce() == nonce)
        else {
            log::error!("operation with nonce {nonce} not found");
            return Err(SchedulerError::TaskExecutionFailed(format!(
                "operation with nonce {nonce} not found"
            )));
        };

        match operation_state {
            OperationState::Deposit(DepositOperationState::MintOrderSent {
                token_id,
                tx_id,
                ..
            }) if token_id == src_token => {
                operation_store.update(
                    operation_id,
                    OperationState::Deposit(DepositOperationState::Minted {
                        token_id: src_token,
                        amount: minted_event.amount,
                        tx_id,
                    }),
                );
            }
            OperationState::Withdrawal(WithdrawalOperationState::RefundMintOrderSent {
                token_id,
                tx_id,
                ..
            }) if token_id == src_token => {
                operation_store.update(
                    operation_id,
                    OperationState::Withdrawal(WithdrawalOperationState::RefundMinted {
                        token_id: src_token,
                        amount: minted_event.amount,
                        tx_id,
                    }),
                );
            }
            OperationState::Deposit(DepositOperationState::MintOrderSent { token_id, .. })
            | OperationState::Withdrawal(WithdrawalOperationState::RefundMintOrderSent {
                token_id,
                ..
            }) => {
                return Err(SchedulerError::TaskExecutionFailed(format!("Operation {operation_id} with nonce {nonce} corresponds to token id {token_id:?} but burnt event was produced by {src_token:?}")));
            }
            _ => {
                return Err(SchedulerError::TaskExecutionFailed(format!(
                    "Operation {operation_id} was in invalid state: {operation_state:?}"
                )));
            }
        }

        log::trace!("Mint order removed");

        Ok(())
    }

    async fn send_mint_transaction(
        config: SharedConfig,
        operation_id: OperationId,
    ) -> Result<(), SchedulerError> {
        log::trace!("Sending mint transaction");

        let mut operation_store = get_operations_store();
        let Some(operation_state) = operation_store.get(operation_id) else {
            log::error!("Operation {operation_id} not found");
            return Ok(());
        };

        let (signed_mint_order, amount, token_id, is_deposit) = match operation_state {
            OperationState::Deposit(DepositOperationState::MintOrderSigned {
                signed_mint_order,
                amount,
                token_id,
            }) => (signed_mint_order, amount, token_id, true),
            OperationState::Withdrawal(WithdrawalOperationState::RefundMintOrderSigned {
                signed_mint_order,
                amount,
                token_id,
            }) => (signed_mint_order, amount, token_id, false),
            _ => {
                log::error!(
                    "deposit request was in incorrect state: {:?}",
                    operation_state
                );
                return Ok(());
            }
        };

        let signer = config.borrow().get_signer().into_scheduler_result()?;
        let sender = signer.get_address().await.into_scheduler_result()?;
        let Some(bridge_contract) = config.borrow().get_bft_bridge_contract() else {
            log::warn!("Bridge contract is not set");
            return Err(SchedulerError::TaskExecutionFailed(
                "Bridge contract is not set".into(),
            ));
        };
        let Ok(evm_params) = config.borrow().get_evm_params() else {
            log::warn!("No evm parameters set");
            return Err(SchedulerError::TaskExecutionFailed(
                "No evm parameters set".into(),
            ));
        };

        let mut tx = bft_events::mint_transaction(
            sender.0,
            bridge_contract.0,
            evm_params.nonce.into(),
            evm_params.gas_price.clone().into(),
            &signed_mint_order.0,
            evm_params.chain_id as _,
        );

        let signature = signer
            .sign_transaction(&(&tx).into())
            .await
            .into_scheduler_result()?;
        tx.r = signature.r.0;
        tx.s = signature.s.0;
        tx.v = signature.v.0;
        tx.hash = tx.hash();

        let client = config.borrow().get_evm_link().get_json_rpc_client();
        let tx_id = client
            .send_raw_transaction(tx)
            .await
            .into_scheduler_result()?;

        if is_deposit {
            operation_store.update(
                operation_id,
                OperationState::Deposit(DepositOperationState::MintOrderSent {
                    token_id,
                    amount,
                    signed_mint_order,
                    tx_id: tx_id.into(),
                }),
            );
        } else {
            operation_store.update(
                operation_id,
                OperationState::Withdrawal(WithdrawalOperationState::RefundMintOrderSent {
                    token_id,
                    amount,
                    signed_mint_order,
                    tx_id: tx_id.into(),
                }),
            );
        }

        log::trace!("Mint transaction sent: {tx_id}");

        Ok(())
    }

    async fn mint_icrc2(
        operation_id: OperationId,
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Result<(), SchedulerError> {
        log::trace!("Minting Icrc2 tokens");

        let mut operation_store = get_operations_store();
        let operation_state = operation_store.get(operation_id);
        let Some(OperationState::Withdrawal(WithdrawalOperationState::Scheduled(burnt_event))) =
            operation_state
        else {
            log::error!(
                "deposit request was in incorrect state: {:?}",
                operation_state
            );
            return Ok(());
        };

        let Some(to_token) =
            Id256::from_slice(&burnt_event.to_token).and_then(|id| id.try_into().ok())
        else {
            log::warn!("Failed to decode token id256 from erc20 minted event");
            return Err(SchedulerError::TaskExecutionFailed(
                "Failed to decode token id256 from erc20 minted event".into(),
            ));
        };

        let Some(recipient) =
            Id256::from_slice(&burnt_event.recipient_id).and_then(|id| id.try_into().ok())
        else {
            log::warn!("Failed to decode recipient id from minted event");
            return Err(SchedulerError::TaskExecutionFailed(
                "Failed to decode recipient id from minted event".into(),
            ));
        };

        // Transfer icrc2 tokens to the recipient.
        let amount = Nat::from(&burnt_event.amount);

        let mint_result = icrc2::mint(to_token, recipient, amount.clone(), true).await;

        match mint_result {
            Ok(Success { tx_id, amount }) => {
                operation_store.update(
                    operation_id,
                    OperationState::Withdrawal(WithdrawalOperationState::Transferred {
                        token: to_token,
                        recipient: recipient.into(),
                        amount,
                        tx_id,
                    }),
                );

                log::trace!("Finished icrc2 mint to principal: {}", recipient);
                Ok(())
            }
            Err(
                e @ IcrcCanisterError::TransferFailed(TransferError::TooOld)
                | e @ IcrcCanisterError::TransferFailed(TransferError::CreatedInFuture { .. })
                | e @ IcrcCanisterError::TransferFailed(TransferError::TemporarilyUnavailable)
                | e @ IcrcCanisterError::TransferFailed(TransferError::GenericError { .. })
                | e @ IcrcCanisterError::CanisterError(RejectionCode::SysTransient, _),
            ) => {
                log::warn!("Failed to perform icrc token mint due to: {e}. Retrying...");
                Err(SchedulerError::TaskExecutionFailed(e.to_string()))
            }
            Err(e) => {
                log::warn!(
                    "Impossible to mint icrc token due to: {e}. Preparing refund MintOrder..."
                );

                // If we pass zero name or symbol, it will not be applied.
                let name = burnt_event.name.try_into().unwrap_or_default();
                let symbol = burnt_event.symbol.try_into().unwrap_or_default();
                let burnt_data = BurntIcrc2Data {
                    sender: recipient,
                    amount: burnt_event.amount,
                    src_token: to_token,
                    recipient_address: burnt_event.sender,
                    erc20_token_address: burnt_event.from_erc20,
                    operation_id: operation_id.nonce(),
                    name,
                    symbol,
                    decimals: burnt_event.decimals,
                    fee_payer: None,
                    approve_after_mint: None,
                };

                operation_store.update(
                    operation_id,
                    OperationState::Withdrawal(WithdrawalOperationState::RefundScheduled(
                        burnt_data,
                    )),
                );

                let task = Self::PrepareMintOrder(operation_id);
                let options = TaskOptions::default()
                    .with_retry_policy(ic_task_scheduler::retry::RetryPolicy::Infinite)
                    .with_backoff_policy(BackoffPolicy::Exponential {
                        secs: 1,
                        multiplier: 4,
                    });

                let task_id = scheduler.append_task(task.into_scheduled(options));
                log::trace!("Appending refund mint order task#{task_id}.");

                Ok(())
            }
        }
    }

    pub async fn update_evm_params(config: SharedConfig) -> Result<(), SchedulerError> {
        let client = config.borrow().get_evm_link().get_json_rpc_client();

        let Some(initial_params) = config.borrow().get_evm_params().ok() else {
            log::warn!("no evm parameters set, unable to update");
            return Err(SchedulerError::TaskExecutionFailed(
                "no evm parameters set".into(),
            ));
        };

        let address = {
            let signer = config.borrow().get_signer().into_scheduler_result()?;
            signer.get_address().await.into_scheduler_result()?
        };

        // Update the EvmParams
        log::trace!("updating evm params");
        let responses = query::batch_query(
            &client,
            &[
                QueryType::Nonce {
                    address: address.into(),
                },
                QueryType::GasPrice,
            ],
        )
        .await
        .into_scheduler_result()?;

        let nonce: U256 = responses
            .get_value_by_id(Id::Str(NONCE_ID.into()))
            .into_scheduler_result()?;
        let gas_price: U256 = responses
            .get_value_by_id(Id::Str(GAS_PRICE_ID.into()))
            .into_scheduler_result()?;

        let params = EvmParams {
            nonce: nonce.0.as_u64(),
            gas_price,
            ..initial_params
        };

        config.borrow_mut().update_evm_params(|p| *p = params);
        log::trace!("evm params updated");

        Ok(())
    }
}
