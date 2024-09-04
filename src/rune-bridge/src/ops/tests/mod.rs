use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use bridge_canister::memory::{memory_by_id, StableMemory};
use bridge_canister::operation_store::OperationsMemory;
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::{SharedConfig, State};
use bridge_did::id256::Id256;
use ic_stable_structures::MemoryId;

use super::*;
use crate::ops::tests::sign_mint_order::TestSigner;

mod await_confirmations;
mod await_inputs;
mod deposit_request;
mod send_mint_order;
mod sign_mint_order;

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

fn test_mint_order() -> MintOrder {
    MintOrder {
        amount: Default::default(),
        sender: Id256::from_evm_address(&H160::from_slice(&[1; 20]), 1),
        src_token: Id256::from_evm_address(&H160::from_slice(&[2; 20]), 1),
        recipient: Default::default(),
        dst_token: Default::default(),
        nonce: 0,
        sender_chain_id: 0,
        recipient_chain_id: 0,
        name: [1; 32],
        symbol: [1; 16],
        decimals: 0,
        approve_spender: Default::default(),
        approve_amount: Default::default(),
        fee_payer: Default::default(),
    }
}

async fn test_signed_order() -> SignedMintOrder {
    test_mint_order()
        .encode_and_sign(&TestSigner::ok())
        .await
        .unwrap()
}

pub mod minter_notification {
    use bridge_canister::bridge::{Operation, OperationAction};
    use bridge_utils::bft_events::{MinterNotificationType, NotifyMinterEventData};
    use candid::Encode;

    use crate::ops::tests::{dst_tokens, test_state, token_address};
    use crate::ops::{RuneBridgeOp, RuneDepositRequestData};

    fn test_deposit_data() -> RuneDepositRequestData {
        RuneDepositRequestData {
            dst_address: token_address(7),
            dst_tokens: dst_tokens(),
            amounts: None,
        }
    }

    fn test_user_data() -> Vec<u8> {
        Encode!(&test_deposit_data()).unwrap()
    }

    #[tokio::test]
    async fn incorrect_type_returns_none() {
        let state = test_state();
        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::Other,
            tx_sender: Default::default(),
            user_data: test_user_data(),
            memo: vec![],
        };

        let result = RuneBridgeOp::on_minter_notification(state, event).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn incorrect_data_returns_none() {
        let state = test_state();
        let mut data = test_user_data();
        data.push(1);
        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: Default::default(),
            user_data: data,
            memo: vec![],
        };

        let result = RuneBridgeOp::on_minter_notification(state, event).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn valid_notification_returns_action() {
        let state = test_state();
        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: Default::default(),
            user_data: test_user_data(),
            memo: vec![],
        };

        let result = RuneBridgeOp::on_minter_notification(state, event).await;
        let expected = test_deposit_data();
        assert_eq!(
            result,
            Some(OperationAction::Create(
                RuneBridgeOp::AwaitInputs {
                    dst_address: expected.dst_address,
                    dst_tokens: expected.dst_tokens,
                    requested_amounts: expected.amounts,
                },
                None,
            ))
        );
    }

    #[tokio::test]
    async fn invalid_memo_length_results_in_none_memo() {
        let state = test_state();
        let memo = vec![2; 20];
        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: Default::default(),
            user_data: test_user_data(),
            memo: memo.clone(),
        };

        let result = RuneBridgeOp::on_minter_notification(state, event).await;
        assert!(matches!(result, Some(OperationAction::Create(_, None))));
    }

    #[tokio::test]
    async fn valid_memo_is_preserved() {
        let state = test_state();
        let memo = vec![2; 32];
        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: Default::default(),
            user_data: test_user_data(),
            memo: memo.clone(),
        };

        let result = RuneBridgeOp::on_minter_notification(state, event).await;
        assert!(
            matches!(result, Some(OperationAction::Create(_, Some(actual_memo))) if actual_memo.to_vec() == memo)
        );
    }
}
