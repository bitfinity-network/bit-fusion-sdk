use async_trait::async_trait;
use bitcoin::bip32::{ChainCode, ChildNumber, DerivationPath, Xpub};
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::{Error, Message, Secp256k1};
use bitcoin::sighash::SighashCache;
use bitcoin::{Address, Network, PublicKey, Transaction, Witness};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{
    sign_with_ecdsa, EcdsaKeyId, SignWithEcdsaArgument,
};
use ord_rs::{BtcTxSigner, Utxo as OrdUtxo};

use crate::interface::{GetAddressError, InscribeError, InscribeResult};

pub const DERIVATION_PATH_PREFIX: u8 = 7;

#[derive(Debug, Clone)]
pub struct IcBtcSigner {
    pub master_key: MasterKey,
    pub network: Network,
}

#[derive(Debug, Clone)]
pub struct MasterKey {
    pub public_key: PublicKey,
    pub chain_code: ChainCode,
    pub key_id: EcdsaKeyId,
}

impl IcBtcSigner {
    pub const DERIVATION_PATH_SIZE: u32 = 21 / 3 * 4;

    pub fn new(master_key: MasterKey, network: Network) -> Self {
        Self {
            master_key,
            network,
        }
    }

    pub fn public_key(&self) -> PublicKey {
        self.master_key.public_key
    }

    pub fn chain_code(&self) -> ChainCode {
        self.master_key.chain_code
    }

    pub fn network(&self) -> Network {
        self.network
    }

    pub fn master_key(&self) -> MasterKey {
        self.master_key.clone()
    }

    /// Verifies an ECDSA signature against a message and a public key.
    pub(crate) fn verify_ecdsa(
        &self,
        signature: &Signature,
        message: &Message,
        public_key: &PublicKey,
    ) -> Result<bool, String> {
        let secp = Secp256k1::verification_only();
        match secp.verify_ecdsa(message, signature, &public_key.inner) {
            Ok(_) => Ok(true),
            Err(err) => Err(format!("Failed to verify ECDSA signature: {}", err)),
        }
    }

    pub async fn sign_transaction_ecdsa(
        &self,
        unsigned_tx: Transaction,
        utxos: &[OrdUtxo],
        own_address: Address,
    ) -> InscribeResult<Transaction> {
        let mut hash = SighashCache::new(unsigned_tx.clone());
        for (index, input) in utxos.iter().enumerate() {
            let sighash = hash
                .p2wpkh_signature_hash(
                    index,
                    &own_address.script_pubkey(),
                    input.amount,
                    bitcoin::EcdsaSighashType::All,
                )
                .map_err(|e| InscribeError::SignatureError(e.to_string()))?;

            log::debug!("Signing transaction and verifying signature");
            let signature = {
                let msg = Message::from(sighash);
                let dp = get_btc_derivation_path(&H160::default())
                    .map_err(InscribeError::DerivationPath)?;

                // sign
                let signature = self
                    .sign_with_ecdsa(msg, &dp)
                    .await
                    .map_err(|e| InscribeError::SignatureError(e.to_string()))?;

                // verify
                self.verify_ecdsa(&signature, &msg, &self.master_key.public_key)
                    .map_err(InscribeError::SignatureError)?;

                signature
            };

            log::debug!("signature: {}", signature.serialize_der());

            // append witness
            let signature = bitcoin::ecdsa::Signature::sighash_all(signature);
            let witness = Witness::p2wpkh(&signature, &self.master_key.public_key.inner);
            *hash
                .witness_mut(index)
                .ok_or(InscribeError::NoSuchUtxo(index.to_string()))? = witness;
        }

        Ok(hash.into_transaction())
    }
}

#[async_trait]
impl BtcTxSigner for IcBtcSigner {
    async fn ecdsa_public_key(&self, derivation_path: &DerivationPath) -> PublicKey {
        let x_public_key = Xpub {
            network: self.network,
            depth: 0,
            parent_fingerprint: Default::default(),
            child_number: ChildNumber::from_normal_idx(0).expect("Failed to create child number"),
            public_key: self.master_key.public_key.inner,
            chain_code: self.master_key.chain_code,
        };
        let public_key = x_public_key
            .derive_pub(&Secp256k1::new(), derivation_path)
            .expect("Failed to derive public key")
            .public_key;

        PublicKey::from(public_key)
    }

    async fn sign_with_ecdsa(
        &self,
        message: Message,
        derivation_path: &DerivationPath,
    ) -> Result<Signature, Error> {
        let request = SignWithEcdsaArgument {
            message_hash: message.as_ref().to_vec(),
            derivation_path: btc_dp_to_ic_dp(derivation_path.clone()),
            key_id: self.master_key.key_id.clone(),
        };

        let response = sign_with_ecdsa(request)
            .await
            .expect("sign_with_ecdsa failed")
            .0;

        Signature::from_compact(&response.signature)
    }

    async fn sign_with_schnorr(
        &self,
        _message: Message,
        _derivation_path: &DerivationPath,
    ) -> Result<bitcoin::secp256k1::schnorr::Signature, Error> {
        Err(Error::IncorrectSignature)
    }
}

/// Retrieves the Bitcoin address for the given derivation path.
pub fn get_bitcoin_address(
    public_key: PublicKey,
    network: Network,
    chain_code: ChainCode,
    eth_address: &H160,
) -> Result<Address, GetAddressError> {
    let x_public_key = Xpub {
        network,
        depth: 0,
        parent_fingerprint: Default::default(),
        child_number: ChildNumber::from_normal_idx(0).map_err(|_| GetAddressError::Derivation)?,
        public_key: public_key.inner,
        chain_code,
    };
    let derivation_path =
        get_btc_derivation_path(eth_address).map_err(|_| GetAddressError::Derivation)?;
    let public_key = x_public_key
        .derive_pub(&Secp256k1::new(), &derivation_path)
        .map_err(|_| GetAddressError::Derivation)?
        .public_key;

    Ok(Address::p2wpkh(&public_key.into(), network)
        .expect("used uncompressed public key to derive address"))
}

pub fn get_ic_derivation_path(eth_address: &H160) -> Vec<Vec<u8>> {
    let mut bytes = vec![DERIVATION_PATH_PREFIX];
    bytes.append(&mut eth_address.0 .0.to_vec());

    let mut dp = vec![];
    for slice in bytes.chunks_exact(3) {
        let mut part = vec![0];
        part.append(&mut slice.to_vec());
        dp.push(part);
    }

    dp
}

pub fn get_btc_derivation_path(eth_address: &H160) -> Result<DerivationPath, GetAddressError> {
    ic_dp_to_btc_dp(&get_ic_derivation_path(eth_address))
}

pub fn ic_dp_to_btc_dp(ic_derivation_path: &[Vec<u8>]) -> Result<DerivationPath, GetAddressError> {
    let mut parts = vec![];
    for part in ic_derivation_path.iter() {
        let child_idx = u32::from_be_bytes(part[..].try_into().unwrap());
        let child = ChildNumber::from_normal_idx(child_idx).unwrap();
        parts.push(child);
    }

    Ok(DerivationPath::from(parts))
}

pub fn btc_dp_to_ic_dp(derivation_path: DerivationPath) -> Vec<Vec<u8>> {
    let vec: Vec<_> = derivation_path.into();
    vec.into_iter()
        .map(|child| u32::from(child).to_be_bytes().to_vec())
        .collect()
}
