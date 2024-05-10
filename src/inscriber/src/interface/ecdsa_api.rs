use candid::CandidType;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{
    self as IcEcdsa, EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyArgument, EcdsaPublicKeyResponse,
    SignWithEcdsaArgument, SignWithEcdsaResponse,
};
use serde::Serialize;

/// Retrieves the public key of this canister at the given derivation path
/// from IC's ECDSA API.
pub async fn ecdsa_public_key(derivation_path: Vec<Vec<u8>>) -> Result<PublicKeyReply, String> {
    let request = EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path,
        key_id: EcdsaKeyIds::TestKeyLocalDevelopment.to_key_id(),
    };

    let (res,): (EcdsaPublicKeyResponse,) = IcEcdsa::ecdsa_public_key(request)
        .await
        .map_err(|e| format!("ecdsa_public_key failed {}", e.1))?;

    Ok(PublicKeyReply {
        public_key_hex: hex::encode(res.public_key),
    })
}

/// Signs a message with an ECDSA key and returns the signature.
pub async fn sign_with_ecdsa(
    derivation_path: Vec<Vec<u8>>,
    message: &str,
) -> Result<SignatureReply, String> {
    let request = SignWithEcdsaArgument {
        message_hash: sha256(message).to_vec(),
        derivation_path,
        key_id: EcdsaKeyIds::TestKeyLocalDevelopment.to_key_id(),
    };

    let (response,): (SignWithEcdsaResponse,) = IcEcdsa::sign_with_ecdsa(request)
        .await
        .map_err(|e| format!("sign_with_ecdsa failed {}", e.1))?;

    Ok(SignatureReply {
        signature_hex: hex::encode(response.signature),
    })
}

/// Verifies an ECDSA signature against a message and a public key.
pub async fn verify_ecdsa(
    signature_hex: &str,
    message: &str,
    public_key_hex: &str,
) -> Result<SignatureVerificationReply, String> {
    let signature_bytes = hex::decode(signature_hex).expect("failed to hex-decode signature");
    let pubkey_bytes = hex::decode(public_key_hex).expect("failed to hex-decode public key");
    let message_bytes = message.as_bytes();

    use k256::ecdsa::signature::Verifier;
    let signature = k256::ecdsa::Signature::try_from(signature_bytes.as_slice())
        .expect("failed to deserialize signature");
    let is_signature_valid = k256::ecdsa::VerifyingKey::from_sec1_bytes(&pubkey_bytes)
        .expect("failed to deserialize sec1 encoding into public key")
        .verify(message_bytes, &signature)
        .is_ok();

    Ok(SignatureVerificationReply { is_signature_valid })
}

fn sha256(input: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(input.as_bytes()).into()
}

enum EcdsaKeyIds {
    #[allow(unused)]
    TestKeyLocalDevelopment,
    #[allow(unused)]
    TestKey1,
    #[allow(unused)]
    ProductionKey1,
}

impl EcdsaKeyIds {
    fn to_key_id(&self) -> EcdsaKeyId {
        EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: match self {
                Self::TestKeyLocalDevelopment => "dfx_test_key",
                Self::TestKey1 => "test_key_1",
                Self::ProductionKey1 => "key_1",
            }
            .to_string(),
        }
    }
}

#[derive(CandidType, Serialize, Debug)]
pub struct PublicKeyReply {
    pub public_key_hex: String,
}

#[derive(CandidType, Serialize, Debug)]
pub struct SignatureReply {
    pub signature_hex: String,
}

#[derive(CandidType, Serialize, Debug)]
pub struct SignatureVerificationReply {
    pub is_signature_valid: bool,
}
