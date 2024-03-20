pub mod bitcoin_api;
pub mod ecdsa_api;
pub mod inscription;

use std::str::FromStr;

use bitcoin::address::Error as AddressError;
use bitcoin::consensus::serialize;
use bitcoin::hashes::Hash;
use bitcoin::{Address, Amount, Network, PublicKey, ScriptBuf, Transaction, Txid};
use hex::ToHex;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ord_rs::wallet::ScriptType;
use ord_rs::{
    CreateCommitTransactionArgs, ExternalSigner, Inscription, OrdError, OrdResult,
    OrdTransactionBuilder, RevealTransactionArgs, Utxo as OrdUtxo, Wallet, WalletType,
};
use serde::de::DeserializeOwned;
use sha2::Digest;

use self::inscription::Protocol;
use crate::canister::ECDSA_KEY_NAME;

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

pub async fn inscribe(
    network: BitcoinNetwork,
    inscription_type: Protocol,
    inscription: String,
    dst_address: Option<String>,
    leftovers_recipient: Option<String>,
) -> OrdResult<(String, String)> {
    // map the network variants
    let bitcoin_network = match network {
        BitcoinNetwork::Mainnet => Network::Bitcoin,
        BitcoinNetwork::Testnet => Network::Testnet,
        BitcoinNetwork::Regtest => Network::Regtest,
    };

    // fetch the arguments for initializing a wallet
    let key_name = ECDSA_KEY_NAME.with(|name| name.borrow().to_string());
    let derivation_path = vec![];
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

    let own_address = public_key_to_p2pkh_address(network, &own_public_key);
    log::info!("Fetching UTXOs...");
    let own_utxos = bitcoin_api::get_utxos(network, own_address.clone())
        .await
        .expect("Failed to retrieve UTXOs for given address")
        .utxos;
    log::trace!("Our own UTXOS: {:#?}", own_utxos);

    let own_address = Address::from_str(&own_address)
        .expect("Failed to parse address")
        .require_network(bitcoin_network)
        .expect("Address belongs to a different network than specified");

    let dst_address = if let Some(addr) = dst_address {
        Address::from_str(&addr)
            .expect("Failed to parse address")
            .require_network(bitcoin_network)
            .expect("Address belongs to a different network than specified")
    } else {
        // Send inscription to canister's own address if `None` is provided
        own_address.clone()
    };

    let leftovers_recipient = if let Some(addr) = leftovers_recipient {
        Address::from_str(&addr)
            .expect("Failed to parse address")
            .require_network(bitcoin_network)
            .expect("Address belongs to a different network than specified")
    } else {
        // Send leftover amounts to canister's address if `None` is provided
        own_address.clone()
    };

    let (commit_tx, reveal_tx) = match inscription_type {
        Protocol::Brc20 => {
            let inscription: ord_rs::Brc20 = serde_json::from_str(&inscription)?;
            build_commit_and_reveal_transactions::<ord_rs::Brc20>(
                &mut builder,
                inscription,
                dst_address.clone(),
                Some(leftovers_recipient),
                &own_utxos,
                bitcoin_network,
            )
            .await?
        }
        Protocol::Nft => {
            let inscription: ord_rs::Nft = serde_json::from_str(&inscription)?;
            build_commit_and_reveal_transactions::<ord_rs::Nft>(
                &mut builder,
                inscription,
                dst_address.clone(),
                Some(leftovers_recipient),
                &own_utxos,
                bitcoin_network,
            )
            .await?
        }
    };

    let commit_tx_bytes = serialize(&commit_tx);
    log::trace!(
        "Signed commit transaction: {}",
        hex::encode(&commit_tx_bytes)
    );

    log::info!("Sending commit transaction...");
    bitcoin_api::send_transaction(network, commit_tx_bytes).await;
    log::info!("Done");

    let reveal_tx_bytes = serialize(&reveal_tx);
    log::trace!(
        "Signed reveal transaction: {}",
        hex::encode(&reveal_tx_bytes)
    );

    log::info!("Sending reveal transaction...");
    bitcoin_api::send_transaction(network, reveal_tx_bytes).await;
    log::info!("Done");

    Ok((commit_tx.txid().encode_hex(), reveal_tx.txid().encode_hex()))
}

async fn build_commit_and_reveal_transactions<T>(
    builder: &mut OrdTransactionBuilder,
    inscription: T,
    recipient_address: Address,
    leftovers_recipient: Option<Address>,
    own_utxos: &[Utxo],
    network: Network,
) -> OrdResult<(Transaction, Transaction)>
where
    T: Inscription + DeserializeOwned,
{
    let mut utxos_to_spend = vec![];
    let mut amount = 0;
    for utxo in own_utxos.iter().rev() {
        amount += utxo.value;
        utxos_to_spend.push(utxo);
    }

    let inputs: Vec<OrdUtxo> = utxos_to_spend
        .into_iter()
        .map(|utxo| OrdUtxo {
            id: Txid::from_raw_hash(
                Hash::from_slice(&utxo.outpoint.txid).expect("Failed to parse tx id"),
            ),
            index: utxo.outpoint.vout,
            amount: Amount::from_sat(amount),
        })
        .collect();

    let leftovers_recipient = if let Some(addr) = leftovers_recipient {
        addr
    } else {
        recipient_address.clone()
    };

    let txin_script_pubkey = ScriptBuf::from_bytes(recipient_address.script_pubkey().into_bytes());

    let Fees {
        commit_fee,
        reveal_fee,
        ..
    } = calculate_transaction_fees(network);

    let commit_tx_args = CreateCommitTransactionArgs {
        inputs,
        inscription,
        leftovers_recipient,
        commit_fee,
        reveal_fee,
        txin_script_pubkey,
    };

    let commit_tx = builder
        .build_commit_transaction(network, commit_tx_args)
        .await?;

    let reveal_tx_args = RevealTransactionArgs {
        input: OrdUtxo {
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

struct Fees {
    pub commit_fee: Amount,
    pub reveal_fee: Amount,
}

// TODO: Verify the source of this
fn calculate_transaction_fees(network: Network) -> Fees {
    match network {
        Network::Bitcoin => Fees {
            commit_fee: Amount::from_sat(15_000),
            reveal_fee: Amount::from_sat(7_000),
        },
        Network::Testnet | Network::Regtest | Network::Signet => Fees {
            commit_fee: Amount::from_sat(2_500),
            reveal_fee: Amount::from_sat(4_700),
        },
        _ => panic!("unknown network"),
    }
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

/// Returns bech32 bitcoin `Address` of this canister at the given derivation path.
pub async fn get_bitcoin_address(
    network: BitcoinNetwork,
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
) -> String {
    // Fetch the public key of the given derivation path.
    let public_key = ecdsa_api::ecdsa_public_key(key_name, derivation_path).await;
    // Compute the bitcoin address.
    public_key_to_bitcoin_address(network, bitcoin::AddressType::P2wpkh, &public_key)
        .expect("Can't convert public key to bitcoin address")
        .to_string()
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

// Compute segwit bitcoin `Address` from `PublicKey`.
fn public_key_to_bitcoin_address(
    bitcoin_network: BitcoinNetwork,
    address_type: bitcoin::AddressType,
    public_key: &[u8],
) -> Result<Address, AddressError> {
    let network = match bitcoin_network {
        BitcoinNetwork::Mainnet => Network::Bitcoin,
        BitcoinNetwork::Regtest => Network::Regtest,
        BitcoinNetwork::Testnet => Network::Testnet,
    };

    match address_type {
        bitcoin::AddressType::P2pkh => {
            let pk = PublicKey::from_slice(public_key).expect("Can't deserialize public key");
            Ok(Address::p2pkh(&pk, network))
        }
        bitcoin::AddressType::P2sh => {
            Address::p2sh(bitcoin::Script::from_bytes(public_key), network)
        }
        bitcoin::AddressType::P2wpkh => {
            let pk = PublicKey::from_slice(public_key).expect("Can't deserialize public key");
            Address::p2wpkh(&pk, network)
        }
        bitcoin::AddressType::P2wsh => Ok(Address::p2wsh(
            bitcoin::Script::from_bytes(public_key),
            network,
        )),
        _ => unimplemented!(),
    }
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
