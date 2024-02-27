pub mod bitcoin_api;
pub mod inscription;
mod types;
mod utils;

use candid::{CandidType, Deserialize};
use ic_cdk::api::management_canister::bitcoin::{
    BitcoinNetwork, GetUtxosResponse, MillisatoshiPerByte,
};
use std::cell::{Cell, RefCell};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(CandidType, Deserialize, Clone, Debug)]
pub enum Error {
    TransactionNotSent(String),
    NoUtxosReturned(String),
    NoBalanceReturned(String),
    CurrentFeePercentilesUnavailable(String),
}

thread_local! {
    // TODO: change to `Mainnet` when in production
    static BITCOIN_NETWORK: Cell<BitcoinNetwork> = Cell::new(BitcoinNetwork::Testnet);

    static ECDSA_DERIVATION_PATH: Vec<Vec<u8>> = vec![];

    static ECDSA_KEY_NAME: RefCell<String> = RefCell::new(String::from(""));
}

#[ic_cdk::init]
pub fn init(network: BitcoinNetwork) {
    BITCOIN_NETWORK.with(|n| n.set(network));

    ECDSA_KEY_NAME.with(|key_name| {
        key_name.replace(String::from(match network {
            BitcoinNetwork::Regtest => "dfx_test_key",
            BitcoinNetwork::Mainnet | BitcoinNetwork::Testnet => "test_key_1",
        }))
    });
}

/// Returns the balance of the given bitcoin address.
#[ic_cdk::update]
pub async fn get_balance(address: String) -> Result<u64> {
    let network = BITCOIN_NETWORK.with(|n| n.get());
    bitcoin_api::get_balance(network, address).await
}

/// Returns the UTXOs of the given bitcoin address.
#[ic_cdk::update]
pub async fn get_utxos(address: String) -> Result<GetUtxosResponse> {
    let network = BITCOIN_NETWORK.with(|n| n.get());
    bitcoin_api::get_utxos(network, address).await
}

/// Returns the 100 fee percentiles measured in millisatoshi/byte.
/// Percentiles are computed from the last 10,000 transactions (if available).
#[ic_cdk::update]
pub async fn get_current_fee_percentiles() -> Result<Vec<MillisatoshiPerByte>> {
    let _network = BITCOIN_NETWORK.with(|n| n.get());
    // TODO: bitcoin_api::get_current_fee_percentiles(network).await
    Ok(Vec::new())
}

/// Returns the P2PKH address of this canister at a specific derivation path.
#[ic_cdk::update]
pub async fn get_p2pkh_address() -> String {
    let _derivation_path = ECDSA_DERIVATION_PATH.with(|d| d.clone());
    let _key_name = ECDSA_KEY_NAME.with(|kn| kn.borrow().to_string());
    let _network = BITCOIN_NETWORK.with(|n| n.get());
    // TODO: bitcoin_wallet::get_p2pkh_address(network, key_name, derivation_path).await
    String::new()
}

/// Sends the given amount of bitcoin from this canister to the given address.
/// Returns the transaction ID.
#[ic_cdk::update]
pub async fn send(_request: types::SendRequest) -> Result<String> {
    let _derivation_path = ECDSA_DERIVATION_PATH.with(|d| d.clone());
    let _network = BITCOIN_NETWORK.with(|n| n.get());
    let _key_name = ECDSA_KEY_NAME.with(|kn| kn.borrow().to_string());
    // TODO:
    // let tx_id = bitcoin_wallet::send(
    //     network,
    //     derivation_path,
    //     key_name,
    //     request.destination_address,
    //     request.amount_in_satoshi,
    // )
    // .await;

    // tx_id.to_string()
    Ok(String::new())
}

// Enable Candid export
ic_cdk::export_candid!();
