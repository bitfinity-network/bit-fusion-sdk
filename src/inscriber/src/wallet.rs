pub mod bitcoin_api;
pub mod ecdsa_api;
pub mod inscription;

use std::str::FromStr;

use bitcoin::consensus::serialize;
use bitcoin::{Address, FeeRate, Network, PublicKey, Transaction};
use hex::ToHex;
use ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use inscription::CommitTransactionArgs;
use ord_rs::wallet::ScriptType;
use ord_rs::{
    CreateCommitTransactionArgs, ExternalSigner, Inscription, OrdError, OrdResult,
    OrdTransactionBuilder, RevealTransactionArgs, TxInput, Wallet, WalletType,
};
use sha2::Digest;

use self::inscription::Protocol;
use crate::constants::{ECDSA_DERIVATION_PATH, ECDSA_KEY_NAME};

struct EcdsaSigner;

#[async_trait::async_trait]
impl ExternalSigner for EcdsaSigner {
    async fn sign(
        &self,
        key_name: String,
        derivation_path: Vec<Vec<u8>>,
        message_hash: Vec<u8>,
    ) -> Vec<u8> {
        ecdsa_api::sign_with_ecdsa(key_name, derivation_path, message_hash).await
    }
}

// WIP
pub async fn inscribe(
    network: BitcoinNetwork,
    inscription_type: Protocol,
    commit_tx_args: CommitTransactionArgs,
    dst_address: Option<String>,
    fee_rate: u64,
) -> OrdResult<(String, String)> {
    // map the network variants
    let bitcoin_network = match network {
        BitcoinNetwork::Mainnet => Network::Bitcoin,
        BitcoinNetwork::Testnet => Network::Testnet,
        BitcoinNetwork::Regtest => Network::Regtest,
    };

    // fetch the arguments for initializing a wallet
    let key_name = ECDSA_KEY_NAME.with(|name| name.borrow().to_string());
    let derivation_path = ECDSA_DERIVATION_PATH.with(|path| path.clone());
    let own_public_key =
        ecdsa_api::ecdsa_public_key(key_name.clone(), derivation_path.clone()).await;

    // initialize a wallet (transaction signer) and a transaction builder
    let wallet_type = WalletType::External {
        signer: Box::new(EcdsaSigner {}),
    };
    let wallet = Wallet::new_with_signer(Some(key_name), Some(derivation_path), wallet_type);
    let mut builder = OrdTransactionBuilder::new(
        PublicKey::from_slice(&own_public_key).map_err(OrdError::PubkeyConversion)?,
        ScriptType::P2TR,
        wallet,
    );

    // Fetch our P2PKH address, and UTXOs.
    let own_address = public_key_to_p2pkh_address(network, &own_public_key);
    ic_cdk::print("Fetching UTXOs...");
    // TODO: refactor to use pagination to retrieve all UTXOs for the given address.
    let _own_utxos = bitcoin_api::get_utxos(network, own_address.clone())
        .await
        .expect("Failed to retrieve UTXOs")
        .0
        .utxos;

    let own_address = Address::from_str(&own_address)
        .unwrap()
        .require_network(bitcoin_network)
        .unwrap();

    let dst_address = if let Some(dst_address) = dst_address {
        Address::from_str(&dst_address)
            .unwrap()
            .require_network(bitcoin_network)
            .unwrap()
    } else {
        // Send inscription to canister's own address if `None` is provided
        own_address.clone()
    };

    // TODO: Schnorr signature support
    // ic_cdk::print("Fetching Schnorr public key...");
    // let raw_schnorr_public_key = schnorr_api::schnorr_public_key(key_name.clone()).await;
    // // Convert the raw public key (sec1 encoded) to a XOnlyPublicKey (BIP 340 encoded)
    // let schnorr_public_key = PublicKey::from_slice(&raw_schnorr_public_key).map_err(OrdError::PubkeyConversion)?;

    let _fee_rate = FeeRate::from_sat_per_vb(fee_rate).unwrap();

    let (commit_tx, reveal_tx) = match inscription_type {
        Protocol::Brc20 => build_commit_and_reveal_transactions(
            &mut builder,
            process_brc20_commit_transaction_args(commit_tx_args),
            dst_address.clone(),
            bitcoin_network,
        )
        .await
        .expect("Failed to build BRC20 commit and reveal transactions"),
        Protocol::Nft => build_commit_and_reveal_transactions(
            &mut builder,
            process_nft_commit_transaction_args(commit_tx_args),
            dst_address.clone(),
            bitcoin_network,
        )
        .await
        .expect("Failed to build NFT commit and reveal transactions"),
    };

    let commit_tx_bytes = serialize(&commit_tx);
    ic_cdk::print(format!(
        "Signed commit transaction: {}",
        hex::encode(&commit_tx_bytes)
    ));

    ic_cdk::print("Sending commit transaction...");
    bitcoin_api::send_transaction(network, commit_tx_bytes).await;
    ic_cdk::print("Done");

    let reveal_tx_bytes = serialize(&reveal_tx);
    ic_cdk::print(format!(
        "Signed reveal transaction: {}",
        hex::encode(&reveal_tx_bytes)
    ));

    ic_cdk::print("Sending reveal transaction...");
    bitcoin_api::send_transaction(network, reveal_tx_bytes).await;
    ic_cdk::print("Done");

    Ok((commit_tx.txid().encode_hex(), reveal_tx.txid().encode_hex()))
}

async fn build_commit_and_reveal_transactions<T>(
    builder: &mut OrdTransactionBuilder,
    args: CreateCommitTransactionArgs<T>,
    recipient_address: Address,
    network: Network,
) -> OrdResult<(Transaction, Transaction)>
where
    T: Inscription,
{
    let commit_tx = builder.build_commit_transaction(network, args).await?;
    let reveal_tx_args = RevealTransactionArgs {
        input: TxInput {
            id: commit_tx.tx.txid(),
            index: 0,
            amount: commit_tx.reveal_balance,
        },
        recipient_address,
        redeem_script: commit_tx.clone().redeem_script,
    };
    let reveal_tx = builder.build_reveal_transaction(reveal_tx_args).await?;
    Ok((commit_tx.tx, reveal_tx))
}

fn process_brc20_commit_transaction_args(
    _brc20_args: CommitTransactionArgs,
) -> CreateCommitTransactionArgs<ord_rs::Brc20> {
    todo!()
}

fn process_nft_commit_transaction_args(
    _nft_args: CommitTransactionArgs,
) -> CreateCommitTransactionArgs<ord_rs::Nft> {
    todo!()
}

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
