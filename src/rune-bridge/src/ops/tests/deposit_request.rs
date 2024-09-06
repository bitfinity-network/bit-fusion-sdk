use std::collections::HashMap;

use bridge_canister::bridge::{Operation, OperationAction};
use bridge_utils::bft_events::{MinterNotificationType, NotifyMinterEventData};
use candid::Encode;

use crate::ops::{tests, RuneBridgeDepositOp, RuneBridgeOp, RuneDepositRequestData};
use crate::rune_info::RuneName;

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

    let result = RuneBridgeOp::on_minter_notification(tests::test_state(), event.clone()).await;
    assert!(result.is_none());

    let event = NotifyMinterEventData {
        notification_type: MinterNotificationType::Other,
        ..event
    };
    let result = RuneBridgeOp::on_minter_notification(tests::test_state(), event.clone()).await;
    assert!(result.is_none());
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

    let result = RuneBridgeOp::on_minter_notification(tests::test_state(), event.clone()).await;
    assert!(result.is_none());

    let event = NotifyMinterEventData {
        user_data: vec![],
        ..event
    };
    let result = RuneBridgeOp::on_minter_notification(tests::test_state(), event.clone()).await;
    assert!(result.is_none());
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

    let result = RuneBridgeOp::on_minter_notification(tests::test_state(), event.clone()).await;
    assert_eq!(
        result,
        Some(OperationAction::Create(
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::AwaitInputs {
                dst_address: tests::sender(),
                dst_tokens: tests::dst_tokens(),
                requested_amounts: None,
            }),
            None
        ))
    )
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

    let result = RuneBridgeOp::on_minter_notification(tests::test_state(), event.clone()).await;
    assert_eq!(
        result,
        Some(OperationAction::Create(
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::AwaitInputs {
                dst_address: tests::sender(),
                dst_tokens: tests::dst_tokens(),
                requested_amounts: Some(amounts),
            }),
            None
        ))
    )
}
