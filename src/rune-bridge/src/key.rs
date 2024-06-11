use std::cell::RefCell;

use async_trait::async_trait;
use bitcoin::bip32::{ChildNumber, DerivationPath, Xpub};
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::{Error, Message, Secp256k1};
use bitcoin::{Address, Network, PublicKey};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{sign_with_ecdsa, SignWithEcdsaArgument};
use ord_rs::wallet::LocalSigner;
use ord_rs::BtcTxSigner;

use crate::interface::GetAddressError;
use crate::state::{MasterKey, State};

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
    ) -> Result<bitcoin::secp256k1::schnorr::Signature, Error> {
        Err(Error::IncorrectSignature)
    }
}

pub enum BtcSignerType {
    Local(LocalSigner),
    Ic(IcBtcSigner),
}

impl BtcSignerType {
    pub async fn get_transit_address(&self, eth_address: &H160, network: Network) -> Address {
        let derivation_path = get_derivation_path(eth_address);
        let public_key = self.ecdsa_public_key(&derivation_path).await;

        Address::p2wpkh(&public_key, network)
            .expect("used uncompressed public key to derive address")
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
    ) -> Result<Signature, Error> {
        match self {
            BtcSignerType::Local(v) => v.sign_with_ecdsa(message, derivation_path).await,
            BtcSignerType::Ic(v) => v.sign_with_ecdsa(message, derivation_path).await,
        }
    }

    async fn sign_with_schnorr(
        &self,
        message: Message,
        derivation_path: &DerivationPath,
    ) -> Result<bitcoin::secp256k1::schnorr::Signature, Error> {
        match self {
            BtcSignerType::Local(v) => v.sign_with_schnorr(message, derivation_path).await,
            BtcSignerType::Ic(v) => v.sign_with_schnorr(message, derivation_path).await,
        }
    }
}

pub fn get_transit_address(
    state: &RefCell<State>,
    eth_address: &H160,
) -> Result<Address, GetAddressError> {
    let state = state.borrow();
    let public_key = state.public_key();
    let chain_code = state.chain_code();
    let x_public_key = Xpub {
        network: state.network(),
        depth: 0,
        parent_fingerprint: Default::default(),
        child_number: ChildNumber::from_normal_idx(0).map_err(|_| GetAddressError::Derivation)?,
        public_key: public_key.inner,
        chain_code,
    };
    let derivation_path = get_derivation_path(eth_address);
    let public_key = x_public_key
        .derive_pub(&Secp256k1::new(), &derivation_path)
        .map_err(|_| GetAddressError::Derivation)?
        .public_key;

    Ok(Address::p2wpkh(&public_key.into(), state.network())
        .expect("used uncompressed public key to derive address"))
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

pub fn get_derivation_path(eth_address: &H160) -> DerivationPath {
    ic_dp_to_derivation_path(&get_derivation_path_ic(eth_address))
}

pub fn ic_dp_to_derivation_path(ic_derivation_path: &[Vec<u8>]) -> DerivationPath {
    let mut parts = vec![];
    for part in ic_derivation_path.iter() {
        let child_idx = u32::from_be_bytes(part[..].try_into().unwrap());
        let child = ChildNumber::from_normal_idx(child_idx).unwrap();
        parts.push(child);
    }

    DerivationPath::from(parts)
}

fn derivation_path_to_ic(derivation_path: DerivationPath) -> Vec<Vec<u8>> {
    let vec: Vec<_> = derivation_path.into();
    vec.into_iter()
        .map(|child| u32::from(child).to_be_bytes().to_vec())
        .collect()
}
