pub mod bitcoin_api;
pub mod ecdsa_api;
pub mod inscription;

use std::str::FromStr;

use bitcoin::consensus::serialize;
use bitcoin::hashes::Hash;
use bitcoin::{Address, Amount, FeeRate, Network, PublicKey, ScriptBuf, Transaction, Txid};
use did::{InscribeError, InscribeResult, InscribeTransactions, InscriptionFees};
use hex::ToHex;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_exports::ic_cdk::print;
use inscription::Nft as CandidNft;
use ord_rs::constants::POSTAGE;
use ord_rs::wallet::ScriptType;
use ord_rs::{
    Brc20, CreateCommitTransaction, CreateCommitTransactionArgs, ExternalSigner, Inscription,
    MultisigConfig, Nft, OrdError, OrdResult, OrdTransactionBuilder, RevealTransactionArgs,
    SignCommitTransactionArgs, Utxo as OrdUtxo, Wallet, WalletType,
};
use serde::de::DeserializeOwned;

use self::inscription::{InscriptionWrapper, Protocol};

const DUMMY_BITCOIN_PUBKEY: &str =
    "02fcf0210771ec96a9e268783c192c9c0d2991d6e957f319b2aa56503ee15fafdd";
const DUMMY_BITCOIN_ADDRESS: &str = "1Q9ioXoxA7xMCHxsMz8z8MMn99kogyo3FS";

#[derive(Clone)]
pub struct EcdsaSigner {
    derivation_path: Vec<Vec<u8>>,
}

#[async_trait::async_trait]
impl ExternalSigner for EcdsaSigner {
    async fn ecdsa_public_key(&self) -> String {
        match ecdsa_api::ecdsa_public_key(self.derivation_path.clone()).await {
            Ok(res) => res.public_key_hex,
            Err(e) => panic!("{e}"),
        }
    }

    async fn sign_with_ecdsa(&self, message: &str) -> String {
        match ecdsa_api::sign_with_ecdsa(self.derivation_path.clone(), message).await {
            Ok(res) => res.signature_hex,
            Err(e) => panic!("{e}"),
        }
    }

    async fn verify_ecdsa(&self, signature_hex: &str, message: &str, public_key_hex: &str) -> bool {
        match ecdsa_api::verify_ecdsa(signature_hex, message, public_key_hex).await {
            Ok(res) => res.is_signature_valid,
            Err(e) => panic!("{e}"),
        }
    }
}

pub struct CanisterWallet {
    bitcoin_network: BitcoinNetwork,
    derivation_path: Vec<Vec<u8>>,
    network: Network,
}

impl CanisterWallet {
    pub fn new(derivation_path: Vec<Vec<u8>>, network: BitcoinNetwork) -> Self {
        Self {
            bitcoin_network: network,
            derivation_path,
            network: Self::map_network(network),
        }
    }

    /// Returns bech32 bitcoin `Address` of this canister.
    pub async fn get_bitcoin_address(&self) -> Address {
        let public_key = match ecdsa_api::ecdsa_public_key(self.derivation_path.clone()).await {
            Ok(res) => res.public_key_hex,
            Err(e) => panic!("{e}"),
        };

        let pk = PublicKey::from_str(&public_key).expect("Can't deserialize public key");
        Self::btc_address_from_public_key(self.bitcoin_network, &pk)
    }

    /// Returns the estimated inscription fees for the given inscription.
    pub async fn get_inscription_fees(
        &self,
        inscription_type: Protocol,
        inscription: String,
        multisig_config: Option<MultisigConfig>,
    ) -> InscribeResult<InscriptionFees> {
        let ecdsa_signer = EcdsaSigner {
            derivation_path: self.derivation_path.clone(),
        };
        let own_utxos = vec![OrdUtxo {
            id: Txid::all_zeros(),
            index: 0,
            amount: Amount::MAX,
        }];
        let dummy_pubkey = PublicKey::from_str(DUMMY_BITCOIN_PUBKEY).unwrap();
        let dummy_address = Address::from_str(DUMMY_BITCOIN_ADDRESS)
            .unwrap()
            .assume_checked();

        // initialize a wallet (transaction signer) and a transaction builder
        let wallet = Wallet::new_with_signer(WalletType::External {
            signer: Box::new(ecdsa_signer),
        });
        // Hardcoded for debugging
        let script_type = ScriptType::P2WSH;
        let mut builder = OrdTransactionBuilder::new(dummy_pubkey, script_type, wallet);

        let dst_address = dummy_address.clone();
        let leftovers_address = dummy_address.clone();
        let fee_rate = self.get_fee_rate().await;

        let inscription = self.build_inscription(inscription_type, inscription)?;
        let commit_tx_result = self.build_commit_transaction(
            &mut builder,
            inscription,
            dst_address.clone(),
            leftovers_address,
            dummy_address,
            &own_utxos,
            fee_rate,
            multisig_config,
        )?;

        Ok(InscriptionFees {
            commit_fee: commit_tx_result.commit_fee.to_sat(),
            reveal_fee: commit_tx_result.reveal_fee.to_sat(),
            postage: POSTAGE,
        })
    }

    /// Handles the inscription flow.
    ///
    /// Returns the transaction IDs for both the commit and reveal transactions.
    pub async fn inscribe(
        &self,
        inscription_type: Protocol,
        inscription: String,
        dst_address: Option<Address>,
        leftovers_address: Address,
        multisig_config: Option<MultisigConfig>,
    ) -> InscribeResult<InscribeTransactions> {
        let ecdsa_signer = EcdsaSigner {
            derivation_path: self.derivation_path.clone(),
        };

        let own_pk = PublicKey::from_str(&ecdsa_signer.ecdsa_public_key().await)
            .map_err(OrdError::PubkeyConversion)?;

        let own_address = Self::btc_address_from_public_key(self.bitcoin_network, &own_pk);

        print("Fetching UTXOs...");
        let own_utxos = bitcoin_api::get_utxos(self.bitcoin_network, own_address.to_string())
            .await
            .map_err(InscribeError::FailedToCollectUtxos)?
            .utxos;

        // initialize a wallet (transaction signer) and a transaction builder
        let wallet = Wallet::new_with_signer(WalletType::External {
            signer: Box::new(ecdsa_signer),
        });
        // Hardcoded for debugging
        let script_type = ScriptType::P2WSH;
        let mut builder = OrdTransactionBuilder::new(own_pk, script_type, wallet);

        let dst_address = dst_address.unwrap_or_else(|| own_address.clone());
        let fee_rate = self.get_fee_rate().await;

        let commit_tx_inputs = self.build_commit_transaction_inputs(&own_utxos);

        let inscription = self.build_inscription(inscription_type, inscription)?;
        let commit_tx_result = self.build_commit_transaction(
            &mut builder,
            inscription,
            dst_address.clone(),
            leftovers_address,
            own_address.clone(),
            &commit_tx_inputs,
            fee_rate,
            multisig_config,
        )?;

        let commit_tx = Self::sign_commit_transaction(
            &mut builder,
            commit_tx_result.unsigned_tx,
            SignCommitTransactionArgs {
                inputs: commit_tx_inputs,
                txin_script_pubkey: own_address.script_pubkey(),
            },
        )
        .await?;

        let reveal_tx = Self::build_reveal_transaction(
            &mut builder,
            &commit_tx,
            commit_tx_result.reveal_balance,
            commit_tx_result.redeem_script,
            dst_address,
        )
        .await?;

        print("Sending commit transaction...");
        bitcoin_api::send_transaction(self.bitcoin_network, serialize(&commit_tx)).await;
        print("Done");

        print("Sending reveal transaction...");
        bitcoin_api::send_transaction(self.bitcoin_network, serialize(&reveal_tx)).await;
        print("Done");

        Ok(InscribeTransactions {
            commit_tx: commit_tx.txid().encode_hex(),
            reveal_tx: reveal_tx.txid().encode_hex(),
        })
    }

    fn build_inscription(
        &self,
        inscription_type: Protocol,
        inscription: String,
    ) -> InscribeResult<InscriptionWrapper> {
        match inscription_type {
            Protocol::Brc20 => {
                let op: Brc20 = serde_json::from_str(&inscription)
                    .map_err(|e| InscribeError::BadInscription(e.to_string()))?;

                let inscription = match op {
                    Brc20::Deploy(data) => Brc20::deploy(data.tick, data.max, data.lim, data.dec),
                    Brc20::Mint(data) => Brc20::mint(data.tick, data.amt),
                    Brc20::Transfer(data) => Brc20::transfer(data.tick, data.amt),
                };

                Ok(inscription.into())
            }
            Protocol::Nft => {
                let data: CandidNft = serde_json::from_str(&inscription)
                    .map_err(|e| InscribeError::BadInscription(e.to_string()))?;
                let inscription = Nft::new(
                    Some(data.content_type.as_bytes().to_vec()),
                    Some(data.body.as_bytes().to_vec()),
                );

                Ok(inscription.into())
            }
        }
    }

    fn build_commit_transaction_inputs(&self, own_utxos: &[Utxo]) -> Vec<OrdUtxo> {
        let mut utxos_to_spend = vec![];
        let mut amount = 0;
        for utxo in own_utxos.iter().rev() {
            amount += utxo.value;
            utxos_to_spend.push(utxo);
        }
        utxos_to_spend
            .clone()
            .into_iter()
            .map(|utxo| OrdUtxo {
                id: Txid::from_raw_hash(
                    Hash::from_slice(&utxo.outpoint.txid).expect("Failed to parse txid"),
                ),
                index: utxo.outpoint.vout,
                amount: Amount::from_sat(amount),
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn build_commit_transaction<T>(
        &self,
        builder: &mut OrdTransactionBuilder,
        inscription: T,
        recipient_address: Address,
        leftovers_address: Address,
        own_address: Address,
        tx_inputs: &[OrdUtxo],
        fee_rate: FeeRate,
        multisig_config: Option<MultisigConfig>,
    ) -> OrdResult<CreateCommitTransaction>
    where
        T: Inscription + DeserializeOwned,
    {
        let commit_tx_args = CreateCommitTransactionArgs {
            inputs: tx_inputs.to_vec(),
            inscription,
            leftovers_recipient: leftovers_address,
            txin_script_pubkey: own_address.script_pubkey(),
            fee_rate,
            multisig_config,
        };

        let commit_tx_result = builder.build_commit_transaction(
            self.network,
            recipient_address.clone(),
            commit_tx_args,
        )?;

        Ok(commit_tx_result)
    }

    async fn sign_commit_transaction(
        builder: &mut OrdTransactionBuilder,
        unsigned_tx: Transaction,
        sign_args: SignCommitTransactionArgs,
    ) -> OrdResult<Transaction> {
        builder
            .sign_commit_transaction(unsigned_tx, sign_args)
            .await
    }

    async fn build_reveal_transaction(
        builder: &mut OrdTransactionBuilder,
        commit_tx: &Transaction,
        reveal_balance: Amount,
        redeem_script: ScriptBuf,
        recipient_address: Address,
    ) -> OrdResult<Transaction> {
        let reveal_tx_args = RevealTransactionArgs {
            input: OrdUtxo {
                id: commit_tx.txid(),
                index: 0,
                amount: reveal_balance,
            },
            recipient_address,
            redeem_script,
        };

        builder.build_reveal_transaction(reveal_tx_args).await
    }

    // Returns bech32 bitcoin `Address` of this canister from given `PublicKey`.
    fn btc_address_from_public_key(network: BitcoinNetwork, pk: &PublicKey) -> Address {
        let network = Self::map_network(network);

        // Compute the bitcoin segwit(bech32) address.
        Address::p2wpkh(pk, network).expect("Can't convert public key to segwit bitcoin address")
    }

    async fn get_fee_rate(&self) -> FeeRate {
        // Get fee percentiles from previous transactions to estimate our own fee.
        let fee_percentiles = bitcoin_api::get_current_fee_percentiles(self.bitcoin_network).await;

        let fee_per_byte = if fee_percentiles.is_empty() {
            // There are no fee percentiles. This case can only happen on a regtest
            // network where there are no non-coinbase transactions. In this case,
            // we use a default of 2000 millisatoshis/byte (i.e. 2 satoshis/byte)
            2000
        } else {
            // Choose the 90th percentile for sending fees.
            fee_percentiles[90]
        };

        FeeRate::from_sat_per_vb(fee_per_byte).expect("Overflow!")
    }

    #[inline]
    pub fn map_network(network: BitcoinNetwork) -> Network {
        match network {
            BitcoinNetwork::Mainnet => Network::Bitcoin,
            BitcoinNetwork::Testnet => Network::Testnet,
            BitcoinNetwork::Regtest => Network::Regtest,
        }
    }
}
