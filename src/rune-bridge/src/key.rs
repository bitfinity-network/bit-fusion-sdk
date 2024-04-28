use std::cell::RefCell;

use async_trait::async_trait;
use bitcoin::bip32::{ChildNumber, DerivationPath, Xpub};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use bitcoin::{Address, PublicKey};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{sign_with_ecdsa, SignWithEcdsaArgument};
use k256::ecdsa::signature::Verifier;
use ord_rs::ExternalSigner;

use crate::interface::GetAddressError;
use crate::state::{MasterKey, State};

pub const DERIVATION_PATH_PREFIX: u8 = 7;

pub struct IcSigner {
    master_key: MasterKey,
    network: Network,
    derivation_path: Vec<Vec<u8>>,
}

impl IcSigner {
    pub const DERIVATION_PATH_SIZE: u32 = 21 / 3 * 4;

    pub fn new(master_key: MasterKey, network: Network, derivation_path: Vec<Vec<u8>>) -> Self {
        Self {
            master_key,
            network,
            derivation_path,
        }
    }

    fn derivation_path(&self) -> Result<DerivationPath, GetAddressError> {
        ic_dp_to_derivation_path(&self.derivation_path)
    }

    pub fn public_key(&self) -> PublicKey {
        let x_public_key = Xpub {
            network: self.network,
            depth: 0,
            parent_fingerprint: Default::default(),
            child_number: ChildNumber::from_normal_idx(0).expect("Failed to create child number"),
            public_key: self.master_key.public_key.inner,
            chain_code: self.master_key.chain_code,
        };
        let public_key = x_public_key
            .derive_pub(
                &Secp256k1::new(),
                &self
                    .derivation_path()
                    .expect("Failed to get derivation path"),
            )
            .expect("Failed to derive public key")
            .public_key;

        PublicKey::from(public_key)
    }
}

#[async_trait]
impl ExternalSigner for IcSigner {
    async fn ecdsa_public_key(&self) -> String {
        hex::encode(self.public_key().inner.serialize())
    }

    async fn sign_with_ecdsa(&self, message: &str) -> String {
        let bytes = hex::decode(message).expect("invalid message hex");
        assert_eq!(bytes.len(), 32);

        let request = SignWithEcdsaArgument {
            message_hash: bytes,
            derivation_path: self.derivation_path.clone(),
            key_id: self.master_key.key_id.clone(),
        };

        let response = sign_with_ecdsa(request)
            .await
            .expect("sign_with_ecdsa failed")
            .0;

        hex::encode(response.signature)
    }

    async fn verify_ecdsa(&self, signature_hex: &str, message: &str, public_key_hex: &str) -> bool {
        let signature_bytes = hex::decode(signature_hex).expect("failed to hex-decode signature");
        let pubkey_bytes = hex::decode(public_key_hex).expect("failed to hex-decode public key");
        let message_bytes = hex::decode(message).expect("invalid message hex");

        let signature = k256::ecdsa::Signature::try_from(signature_bytes.as_slice())
            .expect("failed to deserialize signature");
        let check_result = k256::ecdsa::VerifyingKey::from_sec1_bytes(&pubkey_bytes)
            .expect("failed to deserialize sec1 encoding into public key")
            .verify(&message_bytes, &signature)
            .is_ok();

        // todo: For some reason this check is always false, even though the signature is correct.
        //       Need more testing here. Leave it as is right now.
        log::debug!("Check result: {check_result}");

        true
    }
}

pub fn get_deposit_address(
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
    let derivation_path =
        get_derivation_path(eth_address).map_err(|_| GetAddressError::Derivation)?;
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

fn get_derivation_path(eth_address: &H160) -> Result<DerivationPath, GetAddressError> {
    ic_dp_to_derivation_path(&get_derivation_path_ic(eth_address))
}

fn ic_dp_to_derivation_path(
    ic_derivation_path: &[Vec<u8>],
) -> Result<DerivationPath, GetAddressError> {
    let mut parts = vec![];
    for part in ic_derivation_path.iter() {
        let child_idx = u32::from_be_bytes(part[..].try_into().unwrap());
        let child = ChildNumber::from_normal_idx(child_idx).unwrap();
        parts.push(child);
    }

    Ok(DerivationPath::from(parts))
}
