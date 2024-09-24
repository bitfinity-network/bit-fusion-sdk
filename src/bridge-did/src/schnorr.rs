//! Management canister interface for schnorr signature
//!
//! To be replaced with the official implementation in ic-cdk (at the moment it is not released yet)

use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Debug)]
pub struct ManagementCanisterSchnorrPublicKeyRequest {
    pub canister_id: Option<Principal>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: SchnorrKeyId,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct ManagementCanisterSchnorrPublicKeyReply {
    pub public_key: Vec<u8>,
    pub chain_code: Vec<u8>,
}

#[derive(CandidType, Serialize, Debug, Clone)]
pub struct SchnorrKeyId {
    pub algorithm: SchnorrAlgorithm,
    pub name: String,
}

#[derive(CandidType, Serialize, Debug)]
pub struct ManagementCanisterSignatureRequest {
    pub message: Vec<u8>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: SchnorrKeyId,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct ManagementCanisterSignatureReply {
    pub signature: Vec<u8>,
}

#[derive(CandidType, Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum SchnorrAlgorithm {
    #[serde(rename = "bip340secp256k1")]
    Bip340Secp256k1,
    #[serde(rename = "ed25519")]
    Ed25519,
}

#[derive(CandidType, Serialize, Deserialize, Debug)]
pub struct PublicKeyReply {
    pub public_key_hex: String,
}

#[derive(CandidType, Serialize, Deserialize, Debug)]
pub struct SignatureReply {
    pub signature_hex: String,
}

#[derive(CandidType, Serialize, Deserialize, Debug)]
pub struct SignatureVerificationReply {
    pub is_signature_valid: bool,
}
