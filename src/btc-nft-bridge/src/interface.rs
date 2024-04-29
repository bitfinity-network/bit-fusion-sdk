use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use inscriber::wallet::CanisterWallet;

pub mod bridge_api;
pub mod store;

/// Retrieves the Bitcoin address for the given derivation path.
pub(crate) async fn get_deposit_address(
    network: BitcoinNetwork,
    derivation_path: Vec<Vec<u8>>,
) -> String {
    CanisterWallet::new(derivation_path, network)
        .get_bitcoin_address()
        .await
        .to_string()
}
