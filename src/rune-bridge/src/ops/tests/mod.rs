use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use bridge_canister::memory::{memory_by_id, StableMemory};
use bridge_canister::operation_store::OperationsMemory;
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::{SharedConfig, State};
use ic_stable_structures::MemoryId;

use super::*;
use crate::state::RuneState;

mod await_confirmations;
mod await_inputs;
mod deposit_request;

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

fn test_rune_state() -> Rc<RefCell<RuneState>> {
    Rc::new(RefCell::new(RuneState::default()))
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

pub mod minter_notification {
    use bridge_canister::bridge::OperationAction;
    use bridge_canister::runtime::service::fetch_logs::BftBridgeEventHandler;
    use bridge_did::event_data::*;
    use candid::Encode;

    use crate::ops::events_handler::RuneEventsHandler;
    use crate::ops::tests::{dst_tokens, test_rune_state, token_address};
    use crate::ops::RuneDepositRequestData;

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

    #[test]
    fn incorrect_type_returns_none() {
        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::Other,
            tx_sender: Default::default(),
            user_data: test_user_data(),
            memo: vec![],
        };

        let handler = RuneEventsHandler::new(test_rune_state());
        let result = handler.on_minter_notification(event);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn incorrect_data_returns_none() {
        let mut data = test_user_data();
        data.push(1);
        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: Default::default(),
            user_data: data,
            memo: vec![],
        };

        let handler = RuneEventsHandler::new(test_rune_state());
        let result = handler.on_minter_notification(event);
        assert!(result.is_none())
    }

    #[tokio::test]
    async fn valid_notification_returns_action() {
        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: Default::default(),
            user_data: test_user_data(),
            memo: vec![],
        };

        let handler = RuneEventsHandler::new(test_rune_state());
        let result = handler.on_minter_notification(event);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn invalid_memo_length_results_in_none_memo() {
        let memo = vec![2; 20];
        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: Default::default(),
            user_data: test_user_data(),
            memo: memo.clone(),
        };

        let handler = RuneEventsHandler::new(test_rune_state());
        let result = handler.on_minter_notification(event);
        assert!(matches!(result, Some(OperationAction::Create(_, None))));
    }

    #[tokio::test]
    async fn valid_memo_is_preserved() {
        let memo = vec![2; 32];
        let event = NotifyMinterEventData {
            notification_type: MinterNotificationType::DepositRequest,
            tx_sender: Default::default(),
            user_data: test_user_data(),
            memo: memo.clone(),
        };

        let handler = RuneEventsHandler::new(test_rune_state());
        let result = handler.on_minter_notification(event);
        assert!(
            matches!(result, Some(OperationAction::Create(_, Some(actual_memo))) if actual_memo.to_vec() == memo)
        );
    }
}
