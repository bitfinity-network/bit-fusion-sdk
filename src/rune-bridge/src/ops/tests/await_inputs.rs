use std::str::FromStr;

use bridge_did::error::Error;
use bridge_did::runes::RuneName;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use snapbox::{assert_data_eq, str};

use crate::core::rune_inputs::mock::TestRuneInputProvider;
use crate::core::rune_inputs::{GetInputsError, RuneInput};
use crate::ops::{RuneBridgeOpImpl, tests};

#[tokio::test]
async fn await_inputs_returns_error_if_no_inputs() {
    let provider = TestRuneInputProvider::empty();
    let result = RuneBridgeOpImpl::await_inputs(
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
    let result = RuneBridgeOpImpl::await_inputs(
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
    let result = RuneBridgeOpImpl::await_inputs(
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
        first_response: "response 1".to_string(),
        another_response: "response 2".to_string(),
    });
    let result = RuneBridgeOpImpl::await_inputs(
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
            "failed to get deposit inputs: rune indexers returned different result for same request: response 1; response 2"
        ]
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
    let result = RuneBridgeOpImpl::await_inputs(
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        Some([(RuneName::from_str("A").unwrap(), 1500)].into()),
    )
    .await;
    let Err(Error::FailedToProgress(message)) = result else {
        panic!("Invalid result: {result:?}");
    };

    assert_data_eq!(
        message,
        str![
            "requested amounts {RuneName(Rune(0)): 1500} are not equal actual amounts {RuneName(Rune(0)): 1000}"
        ]
    );
}

#[tokio::test]
async fn await_inputs_returns_error_if_wrong_amounts_multiple_utxos() {
    let inputs = [rune_input("A", 1000), rune_input("B", 2000)];
    let provider = TestRuneInputProvider::with_inputs(&inputs);
    let result = RuneBridgeOpImpl::await_inputs(
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        Some(
            [
                (RuneName::from_str("A").unwrap(), 1000),
                (RuneName::from_str("B").unwrap(), 2500),
            ]
            .into(),
        ),
    )
    .await;
    let Err(Error::FailedToProgress(message)) = result else {
        panic!("Invalid result: {result:?}");
    };

    assert_data_eq!(
        message,
        str!["requested amounts {[..]} are not equal actual amounts {[..]}"]
    );
}

#[tokio::test]
async fn await_inputs_cannot_progress_if_requested_less_than_actual_single_utxo() {
    let input = rune_input("A", 1000);
    let provider = TestRuneInputProvider::with_input(input.clone());
    let result = RuneBridgeOpImpl::await_inputs(
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        Some([(RuneName::from_str("A").unwrap(), 500)].into()),
    )
    .await;
    let Err(Error::CannotProgress(message)) = result else {
        panic!("Invalid result: {result:?}");
    };

    assert_data_eq!(
        message,
        str![
            "requested amounts {RuneName(Rune(0)): 500} cannot be equal actual amounts {RuneName(Rune(0)): 1000}"
        ]
    );
}

#[tokio::test]
async fn await_inputs_cannot_progress_if_requested_less_than_actual_multiple_utxos() {
    let inputs = [rune_input("A", 1000), rune_input("B", 2000)];
    let provider = TestRuneInputProvider::with_inputs(&inputs);
    let result = RuneBridgeOpImpl::await_inputs(
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        Some(
            [
                (RuneName::from_str("A").unwrap(), 1000),
                (RuneName::from_str("B").unwrap(), 1500),
            ]
            .into(),
        ),
    )
    .await;
    let Err(Error::CannotProgress(message)) = result else {
        panic!("Invalid result: {result:?}");
    };

    assert_data_eq!(
        message,
        str!["requested amounts {[..]} cannot be equal actual amounts {[..]}"]
    );
}

#[tokio::test]
async fn await_inputs_cannot_progress_if_exists_utxo_that_is_not_requested() {
    let inputs = [rune_input("A", 1000), rune_input("B", 2000)];
    let provider = TestRuneInputProvider::with_inputs(&inputs);
    let result = RuneBridgeOpImpl::await_inputs(
        tests::test_state(),
        &provider,
        tests::sender(),
        tests::dst_tokens(),
        Some([(RuneName::from_str("A").unwrap(), 1000)].into()),
    )
    .await;
    let Err(Error::CannotProgress(message)) = result else {
        panic!("Invalid result: {result:?}");
    };

    assert_data_eq!(
        message,
        str!["requested amounts {RuneName(Rune(0)): 1000} cannot be equal actual amounts {[..]}"]
    );
}

#[tokio::test]
async fn await_inputs_returns_error_if_no_token_address() {
    let inputs = [rune_input("A", 1000)];
    let provider = TestRuneInputProvider::with_inputs(&inputs);
    let result = RuneBridgeOpImpl::await_inputs(
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
