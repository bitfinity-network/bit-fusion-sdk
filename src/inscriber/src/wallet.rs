pub mod bitcoin_api;
pub mod ecdsa_api;

use ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ord_rs::OrdResult;
use sha2::Digest;

// WIP
pub async fn inscribe(
    _network: BitcoinNetwork,
    _commit_tx_args: Vec<u8>,
    _dst_address: Option<String>,
    _fee_rate: u64,
) -> OrdResult<(String, String)> {
    todo!()
    // // TODO: Refactor
    // let mut builder = OrdTransactionBuilder::p2wsh(PrivateKey::from_wif("").unwrap()); // TEMPORARY
    // let key_name = ECDSA_KEY_NAME.with(|name| name.borrow().to_string());
    // let derivation_path = vec![];

    // // Fetch our public key, P2PKH address, and UTXOs.
    // let own_public_key =
    //     ecdsa_api::ecdsa_public_key(key_name.clone(), derivation_path.clone()).await;
    // let own_address = public_key_to_p2pkh_address(network, &own_public_key);

    // let own_address = Address::from_str(&own_address)
    //     .unwrap()
    //     .require_network(Network::Regtest)
    //     .unwrap();

    // let dst_address = if let Some(dst_address) = dst_address {
    //     Address::from_str(&dst_address)
    //         .unwrap()
    //         .require_network(Network::Regtest)
    //         .unwrap()
    // } else {
    //     // Send inscription to canister's own address if none is provided
    //     own_address.clone()
    // };

    // let (commit_tx, reveal_tx) =
    //     commit_and_reveal(&mut builder, commit_tx_args, dst_address.clone())
    //         .await
    //         .expect("Failed to build commit and reveal transactions");

    // let commit_tx_bytes = serialize(&commit_tx);
    // ic_cdk::print(format!(
    //     "Signed commit transaction: {}",
    //     hex::encode(&commit_tx_bytes)
    // ));

    // ic_cdk::print("Sending commit transaction...");
    // bitcoin_api::send_transaction(network, commit_tx_bytes).await;
    // ic_cdk::print("Done");

    // let reveal_tx_bytes = serialize(&reveal_tx);
    // ic_cdk::print(format!(
    //     "Signed reveal transaction: {}",
    //     hex::encode(&reveal_tx_bytes)
    // ));

    // ic_cdk::print("Sending reveal transaction...");
    // bitcoin_api::send_transaction(network, reveal_tx_bytes).await;
    // ic_cdk::print("Done");

    // Ok((commit_tx.txid().encode_hex(), reveal_tx.txid().encode_hex()))
}

// async fn commit_and_reveal<T>(
//     builder: &mut OrdTransactionBuilder,
//     args: CreateCommitTransactionArgs<T>,
//     recipient_address: Address,
// ) -> OrdResult<(Transaction, Transaction)>
// where
//     T: Inscription,
// {
//     let commit_tx = builder.build_commit_transaction(args)?;
//     let reveal_tx_args = RevealTransactionArgs {
//         input: TxInput {
//             id: commit_tx.tx.txid(),
//             index: 0,
//             amount: commit_tx.reveal_balance,
//         },
//         recipient_address,
//         redeem_script: commit_tx.clone().redeem_script,
//     };
//     let reveal_tx = builder.build_reveal_transaction(reveal_tx_args)?;
//     Ok((commit_tx.tx, reveal_tx))
// }

/// Returns the P2PKH address of this canister at the given derivation path.
/// We use this to generate payment addresses
pub async fn get_p2pkh_address(
    network: BitcoinNetwork,
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
) -> String {
    // Fetch the public key of the given derivation path.
    let public_key = ecdsa_api::ecdsa_public_key(key_name, derivation_path).await;

    // Compute the address.
    public_key_to_p2pkh_address(network, &public_key)
}

// Converts a public key to a P2PKH address.
fn public_key_to_p2pkh_address(network: BitcoinNetwork, public_key: &[u8]) -> String {
    // SHA-256 & RIPEMD-160
    let result = ripemd160(&sha256(public_key));

    let prefix = match network {
        BitcoinNetwork::Testnet | BitcoinNetwork::Regtest => 0x6f,
        BitcoinNetwork::Mainnet => 0x00,
    };
    let mut data_with_prefix = vec![prefix];
    data_with_prefix.extend(result);

    let checksum = &sha256(&sha256(&data_with_prefix.clone()))[..4];

    let mut full_address = data_with_prefix;
    full_address.extend(checksum);

    bs58::encode(full_address).into_string()
}

fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
fn ripemd160(data: &[u8]) -> Vec<u8> {
    let mut hasher = ripemd::Ripemd160::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
