use std::cell::RefCell;

use async_trait::async_trait;
use bitcoin::address::Error as BitcoinAddressError;
use bitcoin::bip32::{ChildNumber, DerivationPath, Error as Bip32Error, Xpub};
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::{Error as Secp256Error, Message, Secp256k1};
use bitcoin::{Address, Network, PublicKey};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{sign_with_ecdsa, SignWithEcdsaArgument};
use ord_rs::wallet::LocalSigner;
use ord_rs::BtcTxSigner;
use thiserror::Error;

use crate::state::{Brc20State, MasterKey};

/// Key result type
pub type KeyResult<T> = Result<T, KeyError>;

/// Key and signer error types
#[derive(Debug, Error)]
pub enum KeyError {
    #[error("bip32 error: {0}")]
    Bip32(#[from] Bip32Error),
    #[error("failed to derive address: {0}")]
    BitcoinAddress(#[from] BitcoinAddressError),
    #[error("invalid derivation path")]
    InvalidDerivationPath,
    #[error("invalid public key")]
    InvalidPublicKey,
    #[error("secp256 error: {0}")]
    Secp256(#[from] Secp256Error),
    #[error("signer not initialized")]
    SignerNotInitialized,
}

pub const DERIVATION_PATH_PREFIX: u8 = 7;

pub struct IcBtcSigner {
    master_key: MasterKey,
    network: Network,
}

impl IcBtcSigner {
    pub const DERIVATION_PATH_SIZE: u32 = 21 / 3 * 4;

    pub fn new(master_key: MasterKey, network: Network) -> Self {
        Self {
            master_key,
            network,
        }
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
            public_key: self.master_key.public_key().expect("invalid pubkey").inner,
            chain_code: self.master_key.chain_code(),
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
    ) -> Result<Signature, Secp256Error> {
        let request = SignWithEcdsaArgument {
            message_hash: message.as_ref().to_vec(),
            derivation_path: derivation_path_to_ic(derivation_path.clone()),
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
    ) -> Result<bitcoin::secp256k1::schnorr::Signature, Secp256Error> {
        Err(Secp256Error::IncorrectSignature)
    }
}

pub enum BtcSignerType {
    Local(LocalSigner),
    Ic(IcBtcSigner),
}

impl BtcSignerType {
    pub async fn get_transit_address(
        &self,
        eth_address: &H160,
        network: Network,
    ) -> KeyResult<Address> {
        let derivation_path = get_derivation_path(eth_address)?;
        let public_key = self.ecdsa_public_key(&derivation_path).await;

        Address::p2wpkh(&public_key, network).map_err(KeyError::BitcoinAddress)
    }
}

#[async_trait]
impl BtcTxSigner for BtcSignerType {
    async fn ecdsa_public_key(&self, derivation_path: &DerivationPath) -> PublicKey {
        match self {
            BtcSignerType::Local(v) => v.ecdsa_public_key(derivation_path).await,
            BtcSignerType::Ic(v) => v.ecdsa_public_key(derivation_path).await,
        }
    }

    async fn sign_with_ecdsa(
        &self,
        message: Message,
        derivation_path: &DerivationPath,
    ) -> Result<Signature, Secp256Error> {
        match self {
            BtcSignerType::Local(v) => v.sign_with_ecdsa(message, derivation_path).await,
            BtcSignerType::Ic(v) => v.sign_with_ecdsa(message, derivation_path).await,
        }
    }

    async fn sign_with_schnorr(
        &self,
        message: Message,
        derivation_path: &DerivationPath,
    ) -> Result<bitcoin::secp256k1::schnorr::Signature, Secp256Error> {
        match self {
            BtcSignerType::Local(v) => v.sign_with_schnorr(message, derivation_path).await,
            BtcSignerType::Ic(v) => v.sign_with_schnorr(message, derivation_path).await,
        }
    }
}

pub fn get_transit_address(state: &RefCell<Brc20State>, eth_address: &H160) -> KeyResult<Address> {
    let state = state.borrow();
    let public_key = state.public_key().ok_or(KeyError::SignerNotInitialized)?;
    let chain_code = state.chain_code().ok_or(KeyError::SignerNotInitialized)?;
    let x_public_key = Xpub {
        network: state.network(),
        depth: 0,
        parent_fingerprint: Default::default(),
        child_number: ChildNumber::from_normal_idx(0)?,
        public_key: public_key.inner,
        chain_code,
    };
    let derivation_path = get_derivation_path(eth_address)?;
    let public_key = x_public_key
        .derive_pub(&Secp256k1::new(), &derivation_path)?
        .public_key;

    Ok(Address::p2wpkh(&public_key.into(), state.network())?)
}

pub fn get_derivation_path_ic(eth_address: &H160) -> Vec<Vec<u8>> {
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

pub fn get_derivation_path(eth_address: &H160) -> KeyResult<DerivationPath> {
    ic_dp_to_derivation_path(&get_derivation_path_ic(eth_address))
}

pub fn ic_dp_to_derivation_path(ic_derivation_path: &[Vec<u8>]) -> KeyResult<DerivationPath> {
    let mut parts = vec![];
    for part in ic_derivation_path.iter() {
        let child_idx = u32::from_be_bytes(
            part[..]
                .try_into()
                .map_err(|_| KeyError::InvalidDerivationPath)?,
        );
        let child = ChildNumber::from_normal_idx(child_idx)?;
        parts.push(child);
    }

    Ok(DerivationPath::from(parts))
}

fn derivation_path_to_ic(derivation_path: DerivationPath) -> Vec<Vec<u8>> {
    let vec: Vec<_> = derivation_path.into();
    vec.into_iter()
        .map(|child| u32::from(child).to_be_bytes().to_vec())
        .collect()
}
