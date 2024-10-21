use std::collections::HashMap;

use bridge_canister::bridge::{Operation, OperationAction};
use bridge_canister::runtime::service::fetch_logs::BftBridgeEventHandler;
use bridge_did::event_data::*;
use bridge_did::runes::RuneName;
use candid::Encode;
use tests::events_handler::RuneEventsHandler;

use crate::ops::{
    tests, RuneBridgeDepositOp, RuneBridgeOp, RuneBridgeOpImpl, RuneDepositRequestData,
};

#[tokio::test]
async fn invalid_notification_type_is_noop() {
    let notification = RuneDepositRequestData {
        dst_address: tests::sender(),
        dst_tokens: tests::dst_tokens(),
        amounts: None,
    };

    let event = NotifyMinterEventData {
        notification_type: MinterNotificationType::RescheduleOperation,
        tx_sender: tests::sender(),
        user_data: Encode!(&notification).unwrap(),
        memo: vec![],
    };

    let handler = RuneEventsHandler::new(tests::test_runtime(), tests::test_rune_state());
    let result = handler.on_minter_notification(event.clone());
    assert!(result.is_err());

    let event = NotifyMinterEventData {
        notification_type: MinterNotificationType::Other,
        ..event
    };
    let result = handler.on_minter_notification(event);
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_notification_payload_is_noop() {
    let notification = RuneDepositRequestData {
        dst_address: tests::sender(),
        dst_tokens: tests::dst_tokens(),
        amounts: None,
    };
    let mut data = Encode!(&notification).unwrap();
    data.push(0);

    let event = NotifyMinterEventData {
        notification_type: MinterNotificationType::DepositRequest,
        tx_sender: tests::sender(),
        user_data: data,
        memo: vec![],
    };

    let handler = RuneEventsHandler::new(tests::test_runtime(), tests::test_rune_state());
    let result = handler.on_minter_notification(event.clone());
    assert!(result.is_err());

    let event = NotifyMinterEventData {
        user_data: vec![],
        ..event
    };
    let result = handler.on_minter_notification(event.clone());
    assert!(result.is_err());
}

#[tokio::test]
async fn deposit_request_creates_correct_operation() {
    let notification = RuneDepositRequestData {
        dst_address: tests::sender(),
        dst_tokens: tests::dst_tokens(),
        amounts: None,
    };
    let data = Encode!(&notification).unwrap();

    let event = NotifyMinterEventData {
        notification_type: MinterNotificationType::DepositRequest,
        tx_sender: tests::sender(),
        user_data: data,
        memo: vec![],
    };

    let handler = RuneEventsHandler::new(tests::test_runtime(), tests::test_rune_state());
    let result = handler.on_minter_notification(event.clone());
    assert!(result.is_ok());
}

#[tokio::test]
async fn deposit_request_adds_amounts_to_operation() {
    let amounts: HashMap<RuneName, u128> = [(tests::rune_name("AAA"), 1000)].into();
    let notification = RuneDepositRequestData {
        dst_address: tests::sender(),
        dst_tokens: tests::dst_tokens(),
        amounts: Some(amounts.clone()),
    };
    let data = Encode!(&notification).unwrap();

    let event = NotifyMinterEventData {
        notification_type: MinterNotificationType::DepositRequest,
        tx_sender: tests::sender(),
        user_data: data,
        memo: vec![],
    };

    let handler = RuneEventsHandler::new(tests::test_runtime(), tests::test_rune_state());
    let result = handler.on_minter_notification(event.clone());
    assert!(result.is_ok());

    let (_, op) = tests::test_state()
        .borrow_mut()
        .operations
        .get_for_address(&tests::sender(), None)
        .first()
        .cloned()
        .unwrap();
    assert_eq!(
        op,
        RuneBridgeOpImpl(RuneBridgeOp::Deposit(RuneBridgeDepositOp::AwaitInputs {
            dst_address: tests::sender(),
            dst_tokens: tests::dst_tokens(),
            requested_amounts: Some(amounts),
        }))
    )
}
