pub mod bitcoin_api;
pub mod ecdsa_api;
pub mod inscription;

use std::str::FromStr;

use bitcoin::consensus::serialize;
use bitcoin::hashes::Hash;
use bitcoin::{Address, Amount, FeeRate, Network, PublicKey, Transaction, Txid};
use hex::ToHex;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_exports::ic_cdk::print;
use inscription::Nft as CandidNft;
use ord_rs::wallet::ScriptType;
use ord_rs::{
    Brc20, CreateCommitTransaction, CreateCommitTransactionArgsV2, ExternalSigner, Inscription,
    MultisigConfig, Nft, OrdError, OrdResult, OrdTransactionBuilder, RevealTransactionArgs,
    Utxo as OrdUtxo, Wallet, WalletType,
};
use serde::de::DeserializeOwned;

use self::inscription::Protocol;

#[derive(Clone)]
pub(crate) struct EcdsaSigner;

#[async_trait::async_trait]
impl ExternalSigner for EcdsaSigner {
    async fn ecdsa_public_key(&self) -> String {
        match ecdsa_api::ecdsa_public_key().await {
            Ok(res) => res.public_key_hex,
            Err(e) => panic!("{e}"),
        }
    }

    async fn sign_with_ecdsa(&self, message: String) -> String {
        match ecdsa_api::sign_with_ecdsa(message).await {
            Ok(res) => res.signature_hex,
            Err(e) => panic!("{e}"),
        }
    }

    async fn verify_ecdsa(
        &self,
        signature_hex: String,
        message: String,
        public_key_hex: String,
    ) -> bool {
        match ecdsa_api::verify_ecdsa(signature_hex, message, public_key_hex).await {
            Ok(res) => res.is_signature_valid,
            Err(e) => panic!("{e}"),
        }
    }
}

/// Returns bech32 bitcoin `Address` of this canister.
pub(crate) async fn get_bitcoin_address(network: BitcoinNetwork) -> Address {
    let public_key = match ecdsa_api::ecdsa_public_key().await {
        Ok(res) => res.public_key_hex,
        Err(e) => panic!("{e}"),
    };

    let pk = PublicKey::from_str(&public_key).expect("Can't deserialize public key");
    btc_address_from_public_key(network, &pk)
}

/// Handles the inscription flow.
///
/// Returns the transaction IDs for both the commit and reveal transactions.
pub(crate) async fn inscribe(
    network: BitcoinNetwork,
    inscription_type: Protocol,
    inscription: String,
    dst_address: Option<String>,
    multisig_config: Option<MultisigConfig>,
) -> OrdResult<(String, String)> {
    // map the network variants
    let bitcoin_network = map_network(network);

    let ecdsa_signer = EcdsaSigner;
    let own_pk = PublicKey::from_str(&ecdsa_signer.ecdsa_public_key().await)
        .map_err(OrdError::PubkeyConversion)?;

    let own_address = btc_address_from_public_key(network, &own_pk);

    print("Fetching UTXOs...");
    let own_utxos = bitcoin_api::get_utxos(network, own_address.to_string())
        .await
        .expect("Failed to retrieve UTXOs for given address")
        .utxos;

    // initialize a wallet (transaction signer) and a transaction builder
    let wallet = Wallet::new_with_signer(WalletType::External {
        signer: Box::new(ecdsa_signer),
    });
    // Hardcoded for debugging
    let script_type = ScriptType::P2WSH;
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

    let commit_tx_result = match inscription_type {
        Protocol::Brc20 => {
            let op: Brc20 = serde_json::from_str(&inscription)?;

            let inscription = match op {
                Brc20::Deploy(data) => Brc20::deploy(data.tick, data.max, data.lim, data.dec),
                Brc20::Mint(data) => Brc20::mint(data.tick, data.amt),
                Brc20::Transfer(data) => Brc20::transfer(data.tick, data.amt),
            };

            build_commit_transaction::<Brc20>(
                &mut builder,
                inscription,
                dst_address.clone(),
                own_address,
                &own_utxos,
                bitcoin_network,
                fee_rate,
                multisig_config,
            )
            .await?
        }
        Protocol::Nft => {
            let data: CandidNft = serde_json::from_str(&inscription)?;
            let inscription = Nft::new(
                Some(data.content_type.as_bytes().to_vec()),
                Some(data.body.as_bytes().to_vec()),
            );

            build_commit_transaction::<Nft>(
                &mut builder,
                inscription,
                dst_address.clone(),
                own_address,
                &own_utxos,
                bitcoin_network,
                fee_rate,
                multisig_config,
            )
            .await?
        }
    };

    let reveal_tx =
        build_reveal_transaction(&mut builder, commit_tx_result.clone(), dst_address).await?;

    print("Sending commit transaction...");
    bitcoin_api::send_transaction(network, serialize(&commit_tx_result.tx)).await;
    print("Done");

    print("Sending reveal transaction...");
    bitcoin_api::send_transaction(network, serialize(&reveal_tx)).await;
    print("Done");

    Ok((
        commit_tx_result.tx.txid().encode_hex(),
        reveal_tx.txid().encode_hex(),
    ))
}

#[allow(clippy::too_many_arguments)]
async fn build_commit_transaction<T>(
    builder: &mut OrdTransactionBuilder,
    inscription: T,
    recipient_address: Address,
    own_address: Address,
    own_utxos: &[Utxo],
    network: Network,
    fee_rate: FeeRate,
    multisig_config: Option<MultisigConfig>,
) -> OrdResult<CreateCommitTransaction>
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
        .clone()
        .into_iter()
        .map(|utxo| OrdUtxo {
            id: Txid::from_raw_hash(
                Hash::from_slice(&utxo.outpoint.txid).expect("Failed to parse txid"),
            ),
            index: utxo.outpoint.vout,
            amount: Amount::from_sat(amount),
        })
        .collect();

    let commit_tx_args = CreateCommitTransactionArgsV2 {
        inputs,
        inscription,
        leftovers_recipient: own_address.clone(),
        txin_script_pubkey: own_address.script_pubkey(),
        fee_rate,
        multisig_config,
    };

    let commit_tx_result = builder
        .build_commit_transaction_v2(network, recipient_address.clone(), commit_tx_args)
        .await?;

    Ok(commit_tx_result)
}

async fn build_reveal_transaction(
    builder: &mut OrdTransactionBuilder,
    commit_tx: CreateCommitTransaction,
    recipient_address: Address,
) -> OrdResult<Transaction> {
    let reveal_tx_args = RevealTransactionArgs {
        input: OrdUtxo {
            id: commit_tx.tx.txid(),
            index: 0,
            amount: commit_tx.reveal_balance,
        },
        recipient_address,
        redeem_script: commit_tx.redeem_script,
    };

    builder.build_reveal_transaction(reveal_tx_args).await
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
