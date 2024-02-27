use ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;

pub async fn get_p2pkh_address(
    _network: BitcoinNetwork,
    _key_name: String,
    _derivation_path: Vec<Vec<u8>>,
) -> String {
    todo!();
}
