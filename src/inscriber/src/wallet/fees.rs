use bitcoin::absolute::LockTime;
use bitcoin::hash_types::Txid;
use bitcoin::hashes::Hash as _;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, FeeRate, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use ord_rs::wallet::ScriptType;
use ord_rs::MultisigConfig;

/// Single ECDSA signature + SIGHASH type size in bytes.
const ECDSA_SIGHASH_SIZE: usize = 72 + 1;
/// Single Schnorr signature + SIGHASH type size for Taproot in bytes.
const SCHNORR_SIGHASH_SIZE: usize = 64 + 1;

pub fn inscription_tranfer_fees(fee_rate: &FeeRate, recipient_address: &Address) -> Amount {
    let tx_out = vec![
        TxOut {
            value: Amount::ONE_SAT,
            script_pubkey: recipient_address.script_pubkey(),
        };
        2
    ];
    let tx_in = vec![
        TxIn {
            previous_output: OutPoint {
                txid: Txid::all_zeros(),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::from_consensus(0xffffffff),
            witness: Witness::new(),
        };
        2
    ];
    let tx_size = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: tx_in,
        output: tx_out,
    }
    .vsize();
    estimate_transaction_fees(ScriptType::P2WSH, tx_size, fee_rate, 2, None)
}

fn estimate_transaction_fees(
    script_type: ScriptType,
    unsigned_tx_size: usize,
    fee_rate: &FeeRate,
    number_of_inputs: usize,
    multisig_config: Option<&MultisigConfig>,
) -> Amount {
    let estimated_sig_size = estimate_signature_size(script_type, multisig_config);
    let total_estimated_tx_size = unsigned_tx_size + (number_of_inputs * estimated_sig_size);

    fee_rate.fee_vb(total_estimated_tx_size as u64).unwrap()
}

/// Estimates the total size of signatures for a given script type and multisig configuration.
fn estimate_signature_size(
    script_type: ScriptType,
    multisig_config: Option<&MultisigConfig>,
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
