use bitcoin::{Amount, FeeRate};
use candid::{CandidType, Deserialize};
use ord_rs::wallet::ScriptType;
use serde::Serialize;

/// Single ECDSA signature + SIGHASH type size in bytes.
const ECDSA_SIGHASH_SIZE: usize = 72 + 1;
/// Single Schnorr signature + SIGHASH type size for Taproot in bytes.
const SCHNORR_SIGHASH_SIZE: usize = 64 + 1;

/// Represents multisig configuration (m of n) for a transaction, if applicable.
/// Encapsulates the number of required signatures and the total number of signatories.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct MultisigConfig {
    /// Number of required signatures (m)
    pub required: usize,
    /// Total number of signatories (n)
    pub total: usize,
}

/// Calculates the estimated transaction fees based on the script type, unsigned transaction size,
/// current network fee rate, and optional multisig configuration.
///
/// # Panics
///
/// This function panics if there's an overflow when calculating the fee.
pub fn calculate_transaction_fees(
    script_type: ScriptType,
    unsigned_tx_size: usize,
    current_fee_rate: FeeRate,
    multisig_config: Option<MultisigConfig>,
) -> Amount {
    let estimated_sig_size = estimate_signature_size(script_type, multisig_config);
    let total_estimated_tx_size = unsigned_tx_size + estimated_sig_size;

    current_fee_rate
        .fee_vb(total_estimated_tx_size as u64)
        .expect("Overflow in calculating transaction fees")
}

/// Estimates the total size of signatures for a given script type and multisig configuration.
fn estimate_signature_size(
    script_type: ScriptType,
    multisig_config: Option<MultisigConfig>,
) -> usize {
    match script_type {
        // For P2WSH, calculate based on the multisig configuration if provided.
        ScriptType::P2WSH => match multisig_config {
            Some(config) => ECDSA_SIGHASH_SIZE * config.required,
            None => ECDSA_SIGHASH_SIZE, // Default to single signature size if no multisig config is provided.
        },
        // For P2TR, use the fixed Schnorr signature size.
        ScriptType::P2TR => SCHNORR_SIGHASH_SIZE,
    }
}
