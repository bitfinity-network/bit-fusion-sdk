use did::{InscribeError, InscribeResult, InscriptionFees};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;

/// Deducts inscription fees from the received UTXOs, and
/// returns the actual UTXOs to be used for the inscription.
pub(crate) fn process_utxos(
    fees: InscriptionFees,
    fetched_utxos: Vec<Utxo>,
) -> InscribeResult<Vec<Utxo>> {
    let InscriptionFees {
        postage,
        commit_fee,
        reveal_fee,
    } = fees;
    let total_fees = postage + commit_fee + reveal_fee;

    let total_utxo_amount = fetched_utxos.iter().map(|utxo| utxo.value).sum::<u64>();

    // Ensure the total UTXO amount covers the total fees.
    if total_utxo_amount < total_fees {
        return Err(InscribeError::InsufficientFundsForFees(format!(
            "Total UTXO amount: {total_utxo_amount}. Total fees required: {total_fees}"
        )));
    }

    // Calculate the remaining UTXO amount after deducting fees.
    let remaining_utxo_amount = total_utxo_amount - total_fees;

    // Select UTXOs to be used for the inscription based on the remaining amount.
    let mut utxos_for_inscription = Vec::new();
    let mut accumulated_amount = 0u64;

    for utxo in fetched_utxos.into_iter() {
        if accumulated_amount < remaining_utxo_amount {
            accumulated_amount += utxo.value;
            utxos_for_inscription.push(utxo);
            // Break the loop if we have accumulated enough UTXOs to cover the inscription.
            if accumulated_amount >= remaining_utxo_amount {
                break;
            }
        }
    }

    Ok(utxos_for_inscription)
}
