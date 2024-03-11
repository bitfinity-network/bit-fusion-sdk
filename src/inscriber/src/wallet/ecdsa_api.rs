use candid::{CandidType, Deserialize, Principal};
use ic_exports::ic_cdk::api::call::call_with_payment;
use ic_exports::ic_cdk::call;
use serde::Serialize;

use crate::constants::SIGN_WITH_ECDSA_COST_CYCLES;

/// Returns the ECDSA public key of this canister at the given derivation path.
pub async fn ecdsa_public_key(key_name: String, derivation_path: Vec<Vec<u8>>) -> Vec<u8> {
    // Retrieve the public key of this canister at the given derivation path
    // from the ECDSA API.
    let res: Result<(ECDSAPublicKeyReply,), _> = call(
        Principal::management_canister(),
        "ecdsa_public_key",
        (ECDSAPublicKey {
            canister_id: None,
            derivation_path,
            key_id: EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: key_name,
            },
        },),
    )
    .await;

    res.unwrap().0.public_key
}

pub async fn sign_with_ecdsa(
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
    message_hash: Vec<u8>,
) -> Vec<u8> {
    let res: Result<(SignWithECDSAReply,), _> = call_with_payment(
        Principal::management_canister(),
        "sign_with_ecdsa",
        (SignWithECDSA {
            message_hash,
            derivation_path,
            key_id: EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: key_name,
            },
        },),
        SIGN_WITH_ECDSA_COST_CYCLES,
    )
    .await;

    res.unwrap().0.signature
}

#[derive(CandidType, Serialize, Deserialize, Debug)]
pub struct ECDSAPublicKeyReply {
    pub public_key: Vec<u8>,
    pub chain_code: Vec<u8>,
}

#[derive(CandidType, Serialize, Deserialize, Debug)]
pub struct EcdsaKeyId {
    pub curve: EcdsaCurve,
    pub name: String,
}

#[derive(CandidType, Serialize, Deserialize, Debug)]
pub enum EcdsaCurve {
    #[serde(rename = "secp256k1")]
    Secp256k1,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct SignWithECDSAReply {
    pub signature: Vec<u8>,
}

#[derive(CandidType, Serialize, Deserialize, Debug)]
pub struct ECDSAPublicKey {
    pub canister_id: Option<Principal>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: EcdsaKeyId,
}

#[derive(CandidType, Serialize, Debug)]
pub struct SignWithECDSA {
    pub message_hash: Vec<u8>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: EcdsaKeyId,
}
