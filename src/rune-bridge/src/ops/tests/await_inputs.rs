use std::str::FromStr;

use bridge_did::error::Error;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ic_exports::ic_kit::MockContext;
use snapbox::{assert_data_eq, str};

use crate::core::rune_inputs::mock::TestRuneInputProvider;
use crate::core::rune_inputs::{GetInputsError, RuneInput};
use crate::core::utxo_handler::RuneToWrap;
use crate::ops::{tests, RuneBridgeOp};
use crate::rune_info::RuneName;

#[tokio::test]
async fn await_inputs_returns_error_if_no_inputs() {
    let provider = TestRuneInputProvider::empty();
    let result = RuneBridgeOp::await_inputs(
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        None,
    )
    .await;
    let Err(Error::FailedToProgress(message)) = result else {
        panic!("Invalid result: {result:?}");
    };

    assert_data_eq!(message, str!["no inputs"])
}

#[tokio::test]
async fn await_inputs_returns_error_if_provider_returns_btc_error() {
    let provider =
        TestRuneInputProvider::err(GetInputsError::BtcAdapter("not available".to_string()));
    let result = RuneBridgeOp::await_inputs(
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        None,
    )
    .await;
    let Err(Error::FailedToProgress(message)) = result else {
        panic!("Invalid result: {result:?}");
    };

    assert_data_eq!(
        message,
        str!["failed to get deposit inputs: failed to connect to IC BTC adapter: not available"]
    )
}

#[tokio::test]
async fn await_inputs_returns_error_if_provider_returns_indexer_error() {
    let provider = TestRuneInputProvider::err(GetInputsError::InsufficientConsensus {
        received_responses: 0,
        required_responses: 1,
        checked_indexers: 0,
    });
    let result = RuneBridgeOp::await_inputs(
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        None,
    )
    .await;
    let Err(Error::FailedToProgress(message)) = result else {
        panic!("Invalid result: {result:?}");
    };

    assert_data_eq!(
        message,
        str![
            "failed to get deposit inputs: insufficient consensus from rune indexers: 0/1 responses received, 0 indexers checked"
        ]
    )
}

#[tokio::test]
async fn await_inputs_returns_error_if_provider_returns_consensus_error() {
    let provider = TestRuneInputProvider::err(GetInputsError::IndexersDisagree {
        indexer_responses: vec![("indexer_name".to_string(), "indexer_response".to_string())],
    });
    let result = RuneBridgeOp::await_inputs(
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        None,
    )
    .await;
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
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
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
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
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
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
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
        tests::test_state(),
        &provider,
        tests::sender(),
        [(RuneName::from_str("C").unwrap(), tests::token_address(5))].into(),
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
    let result = RuneBridgeOp::await_inputs(
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        None,
    )
    .await;
    assert_eq!(
        result,
        Ok(RuneBridgeOp::AwaitConfirmations {
            dst_address: tests::sender(),
            utxo: input.utxo,
            runes_to_wrap: vec![RuneToWrap {
                rune_info: provider.rune_info(&RuneName::from_str("A").unwrap()),
                amount: 1000,
                wrapped_address: tests::token_address(3),
            }],
        })
    );
}

#[tokio::test]
async fn await_inputs_returns_correct_operation_multiple_inputs() {
    MockContext::new().inject();

    let inputs = vec![rune_input("A", 1000), rune_input("B", 2000)];
    let provider = TestRuneInputProvider::with_inputs(&inputs);
    let state = tests::test_state();
    let result = RuneBridgeOp::await_inputs(
        state.clone(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        None,
    )
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

    assert_eq!(wallet_address, tests::sender());
}
