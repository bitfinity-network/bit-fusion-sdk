use std::collections::HashMap;

use bridge_canister::bridge::{Operation, OperationAction, OperationContext};
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::op_id::OperationId;
use bridge_did::order::{MintOrder, SignedMintOrder};
use bridge_utils::bft_events::{
    BurntEventData, MintedEventData, MinterNotificationType, NotifyMinterEventData,
};
use candid::{CandidType, Decode, Deserialize};
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_task_scheduler::task::TaskOptions;
use serde::Serialize;

use crate::canister::{get_rune_state, get_runtime};
use crate::core::deposit::RuneDeposit;
use crate::core::rune_inputs::RuneInputProvider;
use crate::core::withdrawal::{DidTransaction, RuneWithdrawalPayload, Withdrawal};
use crate::rune_info::{RuneInfo, RuneName};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone, PartialEq, Eq)]
pub enum RuneBridgeOp {
    // Deposit
    AwaitInputs {
        dst_address: H160,
        dst_tokens: HashMap<RuneName, H160>,
        requested_amounts: Option<HashMap<RuneName, u128>>,
    },
    AwaitConfirmations {
        dst_address: H160,
        utxo: Utxo,
        runes_to_wrap: Vec<RuneToWrap>,
    },
    SignMintOrder {
        dst_address: H160,
        mint_order: MintOrder,
    },
    SendMintOrder {
        dst_address: H160,
        order: SignedMintOrder,
    },
    ConfirmMintOrder {
        dst_address: H160,
        order: SignedMintOrder,
        tx_id: H256,
    },
    MintOrderConfirmed {
        data: MintedEventData,
    },

    // Withdraw
    CreateTransaction {
        payload: RuneWithdrawalPayload,
    },
    SendTransaction {
        from_address: H160,
        transaction: DidTransaction,
    },
    TransactionSent {
        from_address: H160,
        transaction: DidTransaction,
    },

    OperationSplit {
        wallet_address: H160,
        new_operation_ids: Vec<OperationId>,
    },
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuneToWrap {
    rune_info: RuneInfo,
    amount: u128,
    wrapped_address: H160,
}

impl Operation for RuneBridgeOp {
    async fn progress(self, id: OperationId, ctx: RuntimeState<Self>) -> BftResult<Self> {
        match self {
            RuneBridgeOp::AwaitInputs {
                dst_address,
                dst_tokens,
                requested_amounts,
            } => {
                log::debug!(
                    "RuneBridgeOp::AwaitInputs {dst_address} {dst_tokens:?} {requested_amounts:?}"
                );
                Self::await_inputs(
                    ctx.clone(),
                    &RuneDeposit::get(ctx),
                    dst_address,
                    dst_tokens,
                    requested_amounts,
                )
                .await
            }
            RuneBridgeOp::AwaitConfirmations {
                dst_address,
                utxo,
                runes_to_wrap,
            } => {
                log::debug!(
                    "RuneBridgeOp::AwaitConfirmations {dst_address} {utxo:?} {runes_to_wrap:?}"
                );
                Self::await_confirmations(ctx, dst_address, utxo, runes_to_wrap).await
            }
            RuneBridgeOp::SignMintOrder {
                dst_address,
                mint_order,
            } => {
                log::debug!("RuneBridgeOp::SignMintOrder {dst_address} {mint_order:?}");
                Self::sign_mint_order(ctx, id.nonce(), dst_address, mint_order).await
            }
            RuneBridgeOp::SendMintOrder { dst_address, order } => {
                log::debug!("RuneBridgeOp::SendMintOrder {dst_address} {order:?}");
                Self::send_mint_order(ctx, dst_address, order).await
            }
            RuneBridgeOp::ConfirmMintOrder { .. } => Err(Error::FailedToProgress(
                "ConfirmMintOrder task should progress only on the Minted EVM event".into(),
            )),
            RuneBridgeOp::MintOrderConfirmed { .. } => Err(Error::FailedToProgress(
                "MintOrderConfirmed task cannot be progressed".into(),
            )),
            RuneBridgeOp::CreateTransaction { payload } => {
                log::debug!("RuneBridgeOp::CreateTransaction {payload:?}");
                Self::create_withdrawal_transaction(payload).await
            }
            RuneBridgeOp::SendTransaction {
                from_address,
                transaction,
            } => {
                log::debug!("RuneBridgeOp::SendTransaction {from_address} {transaction:?}");
                Self::send_transaction(from_address, transaction).await
            }
            RuneBridgeOp::TransactionSent { .. } => Err(Error::FailedToProgress(
                "TransactionSent task cannot be progressed".into(),
            )),
            RuneBridgeOp::OperationSplit {
                wallet_address,
                new_operation_ids,
            } => {
                log::debug!("RuneBridgeOp::OperationSplit {wallet_address} {new_operation_ids:?}");
                Self::schedule_operation_split(ctx, new_operation_ids).await
            }
        }
    }

    fn is_complete(&self) -> bool {
        match self {
            RuneBridgeOp::AwaitInputs { .. } => false,
            RuneBridgeOp::AwaitConfirmations { .. } => false,
            RuneBridgeOp::SignMintOrder { .. } => false,
            RuneBridgeOp::SendMintOrder { .. } => false,
            RuneBridgeOp::ConfirmMintOrder { .. } => false,
            RuneBridgeOp::MintOrderConfirmed { .. } => true,
            RuneBridgeOp::CreateTransaction { .. } => false,
            RuneBridgeOp::SendTransaction { .. } => false,
            RuneBridgeOp::TransactionSent { .. } => true,
            RuneBridgeOp::OperationSplit { .. } => false,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match self {
            RuneBridgeOp::AwaitInputs { dst_address, .. } => dst_address.clone(),
            RuneBridgeOp::AwaitConfirmations { dst_address, .. } => dst_address.clone(),
            RuneBridgeOp::SignMintOrder { dst_address, .. } => dst_address.clone(),
            RuneBridgeOp::SendMintOrder { dst_address, .. } => dst_address.clone(),
            RuneBridgeOp::ConfirmMintOrder { dst_address, .. } => dst_address.clone(),
            RuneBridgeOp::MintOrderConfirmed { data } => data.recipient.clone(),
            RuneBridgeOp::CreateTransaction { payload } => payload.sender.clone(),
            RuneBridgeOp::SendTransaction { from_address, .. } => from_address.clone(),
            RuneBridgeOp::TransactionSent { from_address, .. } => from_address.clone(),
            RuneBridgeOp::OperationSplit { wallet_address, .. } => wallet_address.clone(),
        }
    }

    fn scheduling_options(&self) -> Option<ic_task_scheduler::task::TaskOptions> {
        match self {
            Self::SendTransaction { .. } | Self::CreateTransaction { .. } => Some(
                TaskOptions::new()
                    .with_fixed_backoff_policy(2)
                    .with_max_retries_policy(10),
            ),
            Self::AwaitInputs { .. }
            | Self::AwaitConfirmations { .. }
            | Self::SignMintOrder { .. }
            | Self::SendMintOrder { .. }
            | Self::ConfirmMintOrder { .. }
            | Self::MintOrderConfirmed { .. }
            | Self::TransactionSent { .. }
            | Self::OperationSplit { .. } => Some(
                TaskOptions::new()
                    .with_max_retries_policy(10)
                    .with_fixed_backoff_policy(5),
            ),
        }
    }

    async fn on_wrapped_token_minted(
        _ctx: RuntimeState<Self>,
        event: MintedEventData,
    ) -> Option<OperationAction<Self>> {
        log::debug!(
            "on_wrapped_token_minted nonce {nonce} {event:?}",
            nonce = event.nonce
        );

        Some(OperationAction::Update {
            nonce: event.nonce,
            update_to: Self::MintOrderConfirmed { data: event },
        })
    }

    async fn on_wrapped_token_burnt(
        _ctx: RuntimeState<Self>,
        event: BurntEventData,
    ) -> Option<OperationAction<Self>> {
        log::debug!("on_wrapped_token_burnt {event:?}");
        let memo = event.memo();
        match RuneWithdrawalPayload::new(event, &get_rune_state().borrow()) {
            Ok(payload) => Some(OperationAction::Create(
                Self::CreateTransaction { payload },
                memo,
            )),
            Err(err) => {
                log::warn!("Invalid withdrawal data: {err:?}");
                None
            }
        }
    }

    async fn on_minter_notification(
        _ctx: RuntimeState<Self>,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self>> {
        log::debug!("on_minter_notification {event:?}");

        match event.notification_type {
            MinterNotificationType::DepositRequest => {
                match Decode!(&event.user_data, RuneDepositRequestData) {
                    Ok(data) => Some(OperationAction::Create(
                        Self::AwaitInputs {
                            dst_address: data.dst_address,
                            dst_tokens: data.dst_tokens,
                            requested_amounts: data.amounts,
                        },
                        event.memo(),
                    )),
                    _ => {
                        log::warn!(
                            "Invalid encoded deposit request: {}",
                            hex::encode(&event.user_data)
                        );
                        None
                    }
                }
            }
            _ => {
                log::warn!(
                    "Unsupported minter notification type: {:?}",
                    event.notification_type
                );
                None
            }
        }
    }
}

impl RuneBridgeOp {
    fn split(state: RuntimeState<Self>, wallet_address: H160, operations: Vec<Self>) -> Self {
        let mut state = state.borrow_mut();
        let ids = operations
            .into_iter()
            .map(|op| state.operations.new_operation(op, None))
            .collect();
        Self::OperationSplit {
            wallet_address,
            new_operation_ids: ids,
        }
    }

    fn split_or_update(
        state: RuntimeState<Self>,
        wallet_address: H160,
        mut operations: Vec<Self>,
    ) -> Self {
        debug_assert!(
            !operations.is_empty(),
            "operations list must contain at least one operation"
        );

        if operations.len() > 1 {
            Self::split(state, wallet_address, operations)
        } else {
            operations.remove(0)
        }
    }

    async fn schedule_operation_split(
        ctx: RuntimeState<Self>,
        operation_ids: Vec<OperationId>,
    ) -> BftResult<Self> {
        let state = ctx.borrow();

        let mut operations = operation_ids
            .into_iter()
            .filter_map(|id| state.operations.get(id).map(|op| (id, op)))
            .collect::<Vec<_>>();

        log::debug!("Splitting operation: {operations:?}");

        let (_, next_op) = operations.pop().ok_or(Error::FailedToProgress(
            "no operations to split".to_string(),
        ))?;

        // schedule the rest of the operations
        for (id, operation) in operations {
            get_runtime().borrow_mut().schedule_operation(id, operation);
        }

        Ok(next_op)
    }

    async fn await_inputs(
        state: RuntimeState<Self>,
        input_provider: &impl RuneInputProvider,
        dst_address: H160,
        dst_tokens: HashMap<RuneName, H160>,
        requested_amounts: Option<HashMap<RuneName, u128>>,
    ) -> BftResult<Self> {
        let inputs = input_provider
            .get_inputs(&dst_address)
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("failed to get deposit inputs: {err}"))
            })?;

        if inputs.is_empty() {
            return Err(Error::FailedToProgress("no inputs".to_string()));
        }

        if let Some(requested) = &requested_amounts {
            let actual = inputs.rune_amounts();
            if actual != *requested {
                return Err(Error::FailedToProgress(format!(
                    "requested amounts {requested:?} are not equal actual amounts {actual:?}"
                )));
            }
        }

        let mut operations = vec![];
        for input in inputs.inputs.iter() {
            let infos = input_provider
                .get_rune_infos(&input.runes)
                .await
                .ok_or_else(|| Error::FailedToProgress("rune info not found".into()))?;
            let mut runes_to_wrap = vec![];
            for (rune_info, amount) in infos.into_iter() {
                let dst_token =
                    dst_tokens
                        .get(&rune_info.name())
                        .ok_or(Error::FailedToProgress(format!(
                            "wrapped token address for rune {} not found",
                            rune_info.name()
                        )))?;
                runes_to_wrap.push(RuneToWrap {
                    rune_info,
                    amount,
                    wrapped_address: dst_token.clone(),
                });
            }

            operations.push(Self::AwaitConfirmations {
                dst_address: dst_address.clone(),
                utxo: input.utxo.clone(),
                runes_to_wrap,
            });
        }

        Ok(Self::split_or_update(
            state.clone(),
            dst_address,
            operations,
        ))
    }

    async fn await_confirmations(
        ctx: RuntimeState<Self>,
        dst_address: H160,
        utxo: Utxo,
        runes_to_wrap: Vec<RuneToWrap>,
    ) -> BftResult<Self> {
        let deposit = RuneDeposit::get(ctx.clone());
        deposit
            .check_confirmations(&dst_address, &[utxo.clone()])
            .await
            .map_err(|err| Error::FailedToProgress(format!("inputs are not confirmed: {err:?}")))?;

        let deposit_runes = runes_to_wrap.iter().map(|rune| rune.rune_info).collect();
        deposit
            .deposit(&utxo, &dst_address, deposit_runes)
            .await
            .map_err(|err| Error::FailedToProgress(format!("{err:?}")))?;

        let operations = runes_to_wrap
            .into_iter()
            .map(|to_wrap| {
                let mint_order = deposit.create_unsigned_mint_order(
                    &dst_address,
                    &to_wrap.wrapped_address,
                    to_wrap.amount,
                    to_wrap.rune_info,
                    0,
                );
                Self::SignMintOrder {
                    dst_address: dst_address.clone(),
                    mint_order,
                }
            })
            .collect();

        Ok(Self::split_or_update(ctx, dst_address, operations))
    }

    async fn sign_mint_order(
        ctx: RuntimeState<Self>,
        nonce: u32,
        dst_address: H160,
        mut mint_order: MintOrder,
    ) -> BftResult<Self> {
        // update nonce
        mint_order.nonce = nonce;

        let deposit = RuneDeposit::get(ctx);
        let signed = deposit
            .sign_mint_order(mint_order)
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot sign mint order: {err:?}")))?;

        Ok(Self::SendMintOrder {
            dst_address,
            order: signed,
        })
    }

    async fn send_mint_order(
        ctx: RuntimeState<Self>,
        dst_address: H160,
        order: SignedMintOrder,
    ) -> BftResult<Self> {
        let tx_id = ctx.send_mint_transaction(&order).await?;
        Ok(Self::ConfirmMintOrder {
            dst_address,
            order,
            tx_id,
        })
    }

    async fn create_withdrawal_transaction(payload: RuneWithdrawalPayload) -> BftResult<Self> {
        let withdraw = Withdrawal::get();
        let from_address = payload.sender.clone();
        let transaction = withdraw
            .create_withdrawal_transaction(payload)
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("cannot create withdrawal transaction: {err:?}"))
            })?;

        Ok(Self::SendTransaction {
            from_address,
            transaction: transaction.into(),
        })
    }

    async fn send_transaction(from_address: H160, transaction: DidTransaction) -> BftResult<Self> {
        let withdraw = Withdrawal::get();
        withdraw
            .send_transaction(transaction.clone().into())
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("failed to send transaction: {err:?}"))
            })?;

        Ok(Self::TransactionSent {
            from_address,
            transaction,
        })
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct RuneDepositRequestData {
    pub dst_address: H160,
    pub dst_tokens: HashMap<RuneName, H160>,
    pub amounts: Option<HashMap<RuneName, u128>>,
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::str::FromStr;

    use bridge_canister::memory::{memory_by_id, StableMemory};
    use bridge_canister::operation_store::OperationsMemory;
    use bridge_canister::runtime::state::config::ConfigStorage;
    use bridge_canister::runtime::state::{SharedConfig, State};
    use candid::Encode;
    use ic_exports::ic_cdk::api::management_canister::bitcoin::Outpoint;
    use ic_exports::ic_kit::MockContext;
    use ic_stable_structures::MemoryId;
    use snapbox::{assert_data_eq, str};

    use super::*;
    use crate::core::rune_inputs::mock::TestRuneInputProvider;
    use crate::core::rune_inputs::{GetInputsError, RuneInput};

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

    fn test_state() -> RuntimeState<RuneBridgeOp> {
        Rc::new(RefCell::new(State::default(op_memory(), config())))
    }

    fn sender() -> H160 {
        H160::from_slice(&[1; 20])
    }

    fn rune_name(name: &str) -> RuneName {
        RuneName::from_str(name).unwrap()
    }

    fn token_address(v: u8) -> H160 {
        H160::from_slice(&[v; 20])
    }

    fn dst_tokens() -> HashMap<RuneName, H160> {
        [
            (rune_name("AAA"), token_address(2)),
            (rune_name("A"), token_address(3)),
            (rune_name("B"), token_address(4)),
        ]
        .into()
    }

    #[tokio::test]
    async fn invalid_notification_type_is_noop() {
        let notification = RuneDepositRequestData {
            dst_address: sender(),
            dst_tokens: dst_tokens(),
            amounts: None,
        };

        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::RescheduleOperation,
            tx_sender: sender(),
            user_data: Encode!(&notification).unwrap(),
            memo: vec![],
        };

        let result = RuneBridgeOp::on_minter_notification(test_state(), event.clone()).await;
        assert!(result.is_none());

        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::Other,
            ..event
        };
        let result = RuneBridgeOp::on_minter_notification(test_state(), event.clone()).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn invalid_notification_payload_is_noop() {
        let notification = RuneDepositRequestData {
            dst_address: sender(),
            dst_tokens: dst_tokens(),
            amounts: None,
        };
        let mut data = Encode!(&notification).unwrap();
        data.push(0);

        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: sender(),
            user_data: data,
            memo: vec![],
        };

        let result = RuneBridgeOp::on_minter_notification(test_state(), event.clone()).await;
        assert!(result.is_none());

        let event = NotifyMinterEventData {
            user_data: vec![],
            ..event
        };
        let result = RuneBridgeOp::on_minter_notification(test_state(), event.clone()).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn deposit_request_creates_correct_operation() {
        let notification = RuneDepositRequestData {
            dst_address: sender(),
            dst_tokens: dst_tokens(),
            amounts: None,
        };
        let data = Encode!(&notification).unwrap();

        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: sender(),
            user_data: data,
            memo: vec![],
        };

        let result = RuneBridgeOp::on_minter_notification(test_state(), event.clone()).await;
        assert_eq!(
            result,
            Some(OperationAction::Create(
                RuneBridgeOp::AwaitInputs {
                    dst_address: sender(),
                    dst_tokens: dst_tokens(),
                    requested_amounts: None,
                },
                None
            ))
        )
    }

    #[tokio::test]
    async fn deposit_request_adds_amounts_to_operation() {
        let amounts: HashMap<RuneName, u128> = [(rune_name("AAA"), 1000)].into();
        let notification = RuneDepositRequestData {
            dst_address: sender(),
            dst_tokens: dst_tokens(),
            amounts: Some(amounts.clone()),
        };
        let data = Encode!(&notification).unwrap();

        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: sender(),
            user_data: data,
            memo: vec![],
        };

        let result = RuneBridgeOp::on_minter_notification(test_state(), event.clone()).await;
        assert_eq!(
            result,
            Some(OperationAction::Create(
                RuneBridgeOp::AwaitInputs {
                    dst_address: sender(),
                    dst_tokens: dst_tokens(),
                    requested_amounts: Some(amounts),
                },
                None
            ))
        )
    }

    #[tokio::test]
    async fn await_inputs_returns_error_if_no_inputs() {
        let provider = TestRuneInputProvider::empty();
        let result =
            RuneBridgeOp::await_inputs(test_state(), &provider, sender(), dst_tokens(), None).await;
        let Err(Error::FailedToProgress(message)) = result else {
            panic!("Invalid result: {result:?}");
        };

        assert_data_eq!(message, str!["no inputs"])
    }

    #[tokio::test]
    async fn await_inputs_returns_error_if_provider_returns_btc_error() {
        let provider =
            TestRuneInputProvider::err(GetInputsError::BtcAdapter("not available".to_string()));
        let result =
            RuneBridgeOp::await_inputs(test_state(), &provider, sender(), dst_tokens(), None).await;
        let Err(Error::FailedToProgress(message)) = result else {
            panic!("Invalid result: {result:?}");
        };

        assert_data_eq!(
            message,
            str![
                "failed to get deposit inputs: failed to connect to IC BTC adapter: not available"
            ]
        )
    }

    #[tokio::test]
    async fn await_inputs_returns_error_if_provider_returns_indexer_error() {
        let provider = TestRuneInputProvider::err(GetInputsError::IndexersNotAvailable {
            checked_indexers: vec!["url".to_string()],
        });
        let result =
            RuneBridgeOp::await_inputs(test_state(), &provider, sender(), dst_tokens(), None).await;
        let Err(Error::FailedToProgress(message)) = result else {
            panic!("Invalid result: {result:?}");
        };

        assert_data_eq!(
            message,
            str![[r#"failed to get deposit inputs: rune indexers are not available: ["url"]"#]]
        )
    }

    #[tokio::test]
    async fn await_inputs_returns_error_if_provider_returns_consensus_error() {
        let provider = TestRuneInputProvider::err(GetInputsError::IndexersDisagree {
            indexer_responses: vec![("indexer_name".to_string(), "indexer_response".to_string())],
        });
        let result =
            RuneBridgeOp::await_inputs(test_state(), &provider, sender(), dst_tokens(), None).await;
        let Err(Error::FailedToProgress(message)) = result else {
            panic!("Invalid result: {result:?}");
        };

        assert_data_eq!(
            message,
            str![[
                r#"failed to get deposit inputs: rune indexers returned different result for same request: [("indexer_name", "indexer_response")]"#
            ]]
        )
    }

    fn rune_input(rune_name: &str, amount: u128) -> RuneInput {
        RuneInput {
            utxo: Utxo {
                outpoint: Outpoint {
                    txid: vec![],
                    vout: 0,
                },
                value: 10_000,
                height: 0,
            },
            runes: [(RuneName::from_str(rune_name).unwrap(), amount)].into(),
        }
    }

    #[tokio::test]
    async fn await_inputs_returns_error_if_wrong_amounts_one_utxo() {
        let input = rune_input("A", 1000);
        let provider = TestRuneInputProvider::with_input(input.clone());
        let result = RuneBridgeOp::await_inputs(
            test_state(),
            &provider,
            sender(),
            dst_tokens(),
            Some([(RuneName::from_str("B").unwrap(), 1000)].into()),
        )
        .await;
        let Err(Error::FailedToProgress(message)) = result else {
            panic!("Invalid result: {result:?}");
        };

        assert_data_eq!(message, str!["requested amounts {RuneName(Rune(1)): 1000} are not equal actual amounts {RuneName(Rune(0)): 1000}"]);

        let input = rune_input("A", 1000);
        let provider = TestRuneInputProvider::with_input(input.clone());
        let result = RuneBridgeOp::await_inputs(
            test_state(),
            &provider,
            sender(),
            dst_tokens(),
            Some([(RuneName::from_str("A").unwrap(), 2000)].into()),
        )
        .await;
        let Err(Error::FailedToProgress(message)) = result else {
            panic!("Invalid result: {result:?}");
        };

        assert_data_eq!(message, str!["requested amounts {RuneName(Rune(0)): 2000} are not equal actual amounts {RuneName(Rune(0)): 1000}"])
    }

    #[tokio::test]
    async fn await_inputs_returns_error_if_wrong_amounts_multiple_utxos() {
        let inputs = [rune_input("A", 1000), rune_input("B", 2000)];
        let provider = TestRuneInputProvider::with_inputs(&inputs);
        let result = RuneBridgeOp::await_inputs(
            test_state(),
            &provider,
            sender(),
            dst_tokens(),
            Some([(RuneName::from_str("A").unwrap(), 1000)].into()),
        )
        .await;
        let Err(Error::FailedToProgress(message)) = result else {
            panic!("Invalid result: {result:?}");
        };

        assert_data_eq!(
            message,
            str!["requested amounts {RuneName(Rune(0)): 1000} are not equal actual amounts [..]"]
        );
    }

    #[tokio::test]
    async fn await_inputs_returns_error_if_no_token_address() {
        let inputs = [rune_input("A", 1000)];
        let provider = TestRuneInputProvider::with_inputs(&inputs);
        let result = RuneBridgeOp::await_inputs(
            test_state(),
            &provider,
            sender(),
            [(RuneName::from_str("C").unwrap(), token_address(5))].into(),
            None,
        )
        .await;
        let Err(Error::FailedToProgress(message)) = result else {
            panic!("Invalid result: {result:?}");
        };

        assert_data_eq!(message, str!["wrapped token address for rune A not found"]);
    }

    #[tokio::test]
    async fn await_inputs_returns_correct_operation_single_input() {
        let input = rune_input("A", 1000);
        let provider = TestRuneInputProvider::with_input(input.clone());
        let result =
            RuneBridgeOp::await_inputs(test_state(), &provider, sender(), dst_tokens(), None).await;
        assert_eq!(
            result,
            Ok(RuneBridgeOp::AwaitConfirmations {
                dst_address: sender(),
                utxo: input.utxo,
                runes_to_wrap: vec![RuneToWrap {
                    rune_info: provider.rune_info(&RuneName::from_str("A").unwrap()),
                    amount: 1000,
                    wrapped_address: token_address(3),
                }],
            })
        );
    }

    #[tokio::test]
    async fn await_inputs_returns_correct_operation_multiple_inputs() {
        MockContext::new().inject();

        let inputs = vec![rune_input("A", 1000), rune_input("B", 2000)];
        let provider = TestRuneInputProvider::with_inputs(&inputs);
        let state = test_state();
        let result =
            RuneBridgeOp::await_inputs(state.clone(), &provider, sender(), dst_tokens(), None)
                .await;

        let Ok(RuneBridgeOp::OperationSplit {
            wallet_address,
            new_operation_ids,
        }) = result
        else {
            panic!("Incorrect operation returned")
        };

        for operation in new_operation_ids {
            assert!(state.borrow().operations.get(operation).is_some());
        }

        assert_eq!(wallet_address, sender());
    }
}
