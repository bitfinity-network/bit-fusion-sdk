pub mod bitcoin_api;
pub mod ecdsa_api;
pub mod fees;
pub mod inscription;

use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::consensus::serialize;
use bitcoin::hashes::Hash;
use bitcoin::script::Builder;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, AddressType, Amount, FeeRate, Network, OutPoint, PublicKey, Script, ScriptBuf,
    Sequence, Transaction, TxIn, TxOut, Txid, Witness,
};
use hex::ToHex;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use inscription::Nft as CandidNft;
use ord_rs::wallet::ScriptType;
use ord_rs::{
    Brc20, CreateCommitTransactionArgs, ExternalSigner, Inscription, Nft, OrdError, OrdResult,
    OrdTransactionBuilder, RevealTransactionArgs, Utxo as OrdUtxo, Wallet, WalletType,
};
use serde::de::DeserializeOwned;

use self::fees::MultisigConfig;
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
    multisig: Option<MultisigConfig>,
) -> OrdResult<(String, String)> {
    // map the network variants
    let bitcoin_network = map_network(network);

    // fetch the arguments for initializing a wallet
    let key_name = ECDSA_KEY_NAME.with(|name| name.borrow().to_string());
    let derivation_path = vec![];
    let own_public_key =
        ecdsa_api::ecdsa_public_key(key_name.clone(), derivation_path.clone()).await;
    let own_pk = PublicKey::from_slice(&own_public_key).map_err(OrdError::PubkeyConversion)?;
    let own_address = btc_address_from_public_key(network, &own_pk);

    log::info!("Fetching UTXOs...");
    let own_utxos = bitcoin_api::get_utxos(network, own_address.to_string())
        .await
        .expect("Failed to retrieve UTXOs for given address")
        .utxos;
    log::trace!("Our own UTXOS: {:#?}", own_utxos);

    // initialize a wallet (transaction signer) and a transaction builder
    let wallet_type = WalletType::External {
        signer: Box::new(EcdsaSigner {}),
    };

    let script_type = match own_address.address_type() {
        Some(addr_type) => match addr_type {
            AddressType::P2pkh | AddressType::P2sh | AddressType::P2wpkh => ScriptType::P2WSH,
            AddressType::P2tr => ScriptType::P2TR,
            _ => ScriptType::P2WSH,
        },
        None => panic!("Unsupported address type!"),
    };

    let wallet = Wallet::new_with_signer(Some(key_name), Some(derivation_path), wallet_type);
    let mut builder = OrdTransactionBuilder::new(own_pk, script_type, wallet);

    let dst_address = if let Some(addr) = dst_address {
        Address::from_str(&addr)
            .expect("Failed to parse address")
            .require_network(bitcoin_network)
            .expect("Address belongs to a different network than specified")
    } else {
        // Send inscription to canister's own address if `None` is provided
        own_address.clone()
    };

    // Get fee percentiles from previous transactions to estimate our own fee.
    let fee_percentiles = bitcoin_api::get_current_fee_percentiles(network).await;

    let fee_per_byte = if fee_percentiles.is_empty() {
        // There are no fee percentiles. This case can only happen on a regtest
        // network where there are no non-coinbase transactions. In this case,
        // we use a default of 2000 millisatoshis/byte (i.e. 2 satoshis/byte)
        2000
    } else {
        // Choose the 90th percentile for sending fees.
        fee_percentiles[90]
    };

    let fee_rate = FeeRate::from_sat_per_vb(fee_per_byte).expect("Overflow!");

    let (commit_tx, reveal_tx) = match inscription_type {
        Protocol::Brc20 => {
            let op: Brc20 = serde_json::from_str(&inscription)?;

            let inscription = match op {
                Brc20::Deploy(data) => Brc20::deploy(data.tick, data.max, data.lim, data.dec),
                Brc20::Mint(data) => Brc20::mint(data.tick, data.amt),
                Brc20::Transfer(data) => Brc20::transfer(data.tick, data.amt),
            };

            build_commit_and_reveal_transactions::<Brc20>(
                &mut builder,
                inscription,
                dst_address.clone(),
                own_address,
                &own_utxos,
                bitcoin_network,
                fee_rate,
                multisig,
                script_type,
            )
            .await?
        }
        Protocol::Nft => {
            let data: CandidNft = serde_json::from_str(&inscription)?;
            let inscription = Nft::new(
                Some(data.content_type.as_bytes().to_vec()),
                Some(data.body.as_bytes().to_vec()),
            );

            build_commit_and_reveal_transactions::<Nft>(
                &mut builder,
                inscription,
                dst_address.clone(),
                own_address,
                &own_utxos,
                bitcoin_network,
                fee_rate,
                multisig,
                script_type,
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

#[allow(clippy::too_many_arguments)]
async fn build_commit_and_reveal_transactions<T>(
    builder: &mut OrdTransactionBuilder,
    inscription: T,
    recipient_address: Address,
    own_address: Address,
    own_utxos: &[Utxo],
    network: Network,
    fee_rate: FeeRate,
    multisig: Option<MultisigConfig>,
    script_type: ScriptType,
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

    let total_spent = Amount::from_sat(amount);

    let inputs: Vec<OrdUtxo> = utxos_to_spend
        .clone()
        .into_iter()
        .map(|utxo| OrdUtxo {
            id: Txid::from_raw_hash(
                Hash::from_slice(&utxo.outpoint.txid).expect("Failed to parse tx id"),
            ),
            index: utxo.outpoint.vout,
            amount: total_spent,
        })
        .collect();

    let leftovers_recipient = own_address.clone();

    let txin_script_pubkey = ScriptBuf::from_bytes(own_address.script_pubkey().into_bytes());

    let unsigned_commit_tx_size = estimate_unsigned_commit_tx_size(
        utxos_to_spend.clone(),
        total_spent,
        txin_script_pubkey.clone(),
    );

    let commit_fee = fees::calculate_transaction_fees(
        script_type,
        unsigned_commit_tx_size,
        fee_rate,
        multisig.clone(),
    );

    let unsigned_reveal_tx_size = estimate_unsigned_reveal_tx_size(
        vec![OutPoint::null()],
        vec![TxOut {
            script_pubkey: recipient_address.script_pubkey(),
            value: Amount::from_sat(0),
        }],
    );

    let reveal_fee =
        fees::calculate_transaction_fees(script_type, unsigned_reveal_tx_size, fee_rate, multisig);

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

pub fn estimate_unsigned_commit_tx_size(
    utxos_to_spend: Vec<&Utxo>,
    amount: Amount,
    txin_script_pubkey: ScriptBuf,
) -> usize {
    let tx_in: Vec<TxIn> = utxos_to_spend
        .into_iter()
        .map(|utxo| TxIn {
            previous_output: OutPoint {
                txid: Txid::from_raw_hash(
                    Hash::from_slice(&utxo.outpoint.txid)
                        .expect("Failed to copy txid byte slice into hash object"),
                ),
                vout: utxo.outpoint.vout,
            },
            sequence: Sequence::ZERO,
            witness: Witness::new(),
            script_sig: Script::new().into(),
        })
        .collect();

    let unsigned_commit_tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: tx_in,
        output: vec![TxOut {
            script_pubkey: txin_script_pubkey.clone(),
            value: amount,
        }],
    };

    unsigned_commit_tx.vsize()
}

/// Create the reveal transaction
fn estimate_unsigned_reveal_tx_size(inputs: Vec<OutPoint>, outputs: Vec<TxOut>) -> usize {
    let unsigned_reveal_tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: inputs
            .iter()
            .map(|outpoint| TxIn {
                previous_output: *outpoint,
                script_sig: Builder::new().into_script(),
                witness: Witness::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            })
            .collect(),
        output: outputs,
    };

    unsigned_reveal_tx.vsize()
}

/// Returns bech32 bitcoin `Address` of this canister at the given derivation path.
pub async fn get_bitcoin_address(
    network: BitcoinNetwork,
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
) -> Address {
    // Fetch the public key of the given derivation path.
    let public_key = ecdsa_api::ecdsa_public_key(key_name, derivation_path).await;
    let pk = PublicKey::from_slice(&public_key).expect("Can't deserialize public key");

    btc_address_from_public_key(network, &pk)
}

// Returns bech32 bitcoin `Address` of this canister from given `PublicKey`.
fn btc_address_from_public_key(network: BitcoinNetwork, pk: &PublicKey) -> Address {
    let network = map_network(network);

    // Compute the bitcoin segwit(bech32) address.
    Address::p2wpkh(pk, network).expect("Can't convert public key to segwit bitcoin address")
}

fn map_network(network: BitcoinNetwork) -> Network {
    match network {
        BitcoinNetwork::Mainnet => Network::Bitcoin,
        BitcoinNetwork::Testnet => Network::Testnet,
        BitcoinNetwork::Regtest => Network::Regtest,
    }
}
