use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::sighash::SighashCache;
use bitcoin::{PublicKey, ScriptBuf, Transaction, Witness};
use did::{InscribeError, InscribeResult};
use ord_rs::{ExternalSigner as _, Utxo};

use super::EcdsaSigner;

pub struct Spender {
    pub pubkey: PublicKey,
    pub script: ScriptBuf,
}

pub struct Signer {
    signer: EcdsaSigner,
}

impl Signer {
    /// Creates a new `Signer` instance.
    pub fn new(signer: EcdsaSigner) -> Self {
        Self { signer }
    }

    /// Signs a transaction with ECDSA.
    pub async fn sign_transaction_ecdsa(
        &self,
        unsigned_tx: Transaction,
        utxos: &[Utxo],
        spender: Spender,
    ) -> InscribeResult<Transaction> {
        let mut hash = SighashCache::new(unsigned_tx.clone());
        for (index, input) in utxos.iter().enumerate() {
            let sighash = hash
                .p2wpkh_signature_hash(
                    index,
                    &spender.script,
                    input.amount,
                    bitcoin::EcdsaSighashType::All,
                )
                .map_err(|e| InscribeError::SignatureError(e.to_string()))?;

            log::debug!("Signing transaction and verifying signature");
            let signature = {
                let msg_hex = hex::encode(sighash);
                // sign
                let sig_hex = self.signer.sign_with_ecdsa(&msg_hex).await;
                let signature = Signature::from_compact(
                    &hex::decode(&sig_hex)
                        .map_err(|e| InscribeError::SignatureError(e.to_string()))?,
                )
                .map_err(|e| InscribeError::SignatureError(e.to_string()))?;

                // verify
                if !self
                    .signer
                    .verify_ecdsa(&sig_hex, &msg_hex, &spender.pubkey.to_string())
                    .await
                {
                    return Err(InscribeError::SignatureError(
                        "signature verification failed".to_string(),
                    ));
                }
                signature
            };

            log::debug!("signature: {}", signature.serialize_der());

            // append witness
            let signature = bitcoin::ecdsa::Signature::sighash_all(signature).into();
            let witness = Witness::p2wpkh(&signature, &spender.pubkey.inner);
            *hash
                .witness_mut(index)
                .ok_or(InscribeError::NoSuchUtxo(index.to_string()))? = witness;
        }

        Ok(hash.into_transaction())
    }
}
