use bridge_did::error::Error;
use bridge_did::runes::{RuneInfo, RuneToWrap};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ordinals::Rune;
use snapbox::{assert_data_eq, str};

use crate::core::utxo_handler::UtxoHandlerError;
use crate::core::utxo_handler::test::TestUtxoHandler;
use crate::ops::{RuneBridgeOpImpl, tests};

fn get_utxo() -> Utxo {
    Utxo {
        outpoint: Outpoint {
            txid: vec![],
            vout: 0,
        },
        value: 0,
        height: 0,
    }
}

fn get_to_wrap(count: usize) -> Vec<RuneToWrap> {
    let mut result = vec![];
    for i in 0..count {
        result.push(RuneToWrap {
            rune_info: RuneInfo {
                name: Rune((100500 + i) as u128).into(),
                decimals: 0,
                block: 0,
                tx: 0,
            },
            amount: 0,
            wrapped_address: H160::from_slice(&[1; 20]),
        })
    }

    result
}

#[tokio::test]
async fn await_confirmations_utxo_not_found() {
    let utxo_handler = TestUtxoHandler::with_error(UtxoHandlerError::UtxoNotFound);
    let result = RuneBridgeOpImpl::await_confirmations(
        tests::test_state(),
        &utxo_handler,
        tests::sender(),
        get_utxo(),
        get_to_wrap(1),
    )
    .await;

    let Err(Error::FailedToProgress(message)) = result else {
        panic!("Wrong result: {result:?}");
    };

    assert_data_eq!(message, str!["requested utxo is not in the main branch"]);
}

#[tokio::test]
async fn await_confirmations_not_confirmed() {
    let utxo_handler = TestUtxoHandler::with_error(UtxoHandlerError::NotConfirmed {
        required_confirmations: 12,
        current_confirmations: 5,
    });
    let result = RuneBridgeOpImpl::await_confirmations(
        tests::test_state(),
        &utxo_handler,
        tests::sender(),
        get_utxo(),
        get_to_wrap(1),
    )
    .await;

    let Err(Error::FailedToProgress(message)) = result else {
        panic!("Wrong result: {result:?}");
    };

    assert_data_eq!(
        message,
        str!["utxo is not confirmed, required 12, currently 5 confirmations"]
    );
}

#[tokio::test]
async fn await_confirmations_btc_adapter_not_available() {
    let utxo_handler =
        TestUtxoHandler::with_error(UtxoHandlerError::BtcAdapter("btc error".to_string()));
    let result = RuneBridgeOpImpl::await_confirmations(
        tests::test_state(),
        &utxo_handler,
        tests::sender(),
        get_utxo(),
        get_to_wrap(1),
    )
    .await;

    let Err(Error::FailedToProgress(message)) = result else {
        panic!("Wrong result: {result:?}");
    };

    assert_data_eq!(
        message,
        str!["failed to connect to IC BTC adapter: btc error"]
    );
}

#[tokio::test]
async fn await_confirmations_utxo_already_used() {
    let utxo_handler = TestUtxoHandler::already_used_utxo();
    let result = RuneBridgeOpImpl::await_confirmations(
        tests::test_state(),
        &utxo_handler,
        tests::sender(),
        get_utxo(),
        get_to_wrap(1),
    )
    .await;

    let Err(Error::FailedToProgress(message)) = result else {
        panic!("Wrong result: {result:?}");
    };

    assert_data_eq!(message, str!["utxo is already used to create mint orders"]);
}
