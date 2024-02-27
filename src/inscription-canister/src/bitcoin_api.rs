use crate::{BitcoinApiError, BitcoinApiResult};
use candid::Principal;
use ic_cdk::api::{
    call::call_with_payment,
    management_canister::bitcoin::{
        BitcoinNetwork, GetBalanceRequest, GetCurrentFeePercentilesRequest, GetUtxosRequest,
        GetUtxosResponse, MillisatoshiPerByte, Satoshi, SendTransactionRequest,
    },
};

// The fees for the various Bitcoin endpoints.
const GET_BALANCE_COST_CYCLES: u64 = 100_000_000;
const GET_UTXOS_COST_CYCLES: u64 = 10_000_000_000;
const GET_CURRENT_FEE_PERCENTILES_CYCLES: u64 = 100_000_000;
const SEND_TRANSACTION_BASE_CYCLES: u64 = 5_000_000_000;
const SEND_TRANSACTION_PER_BYTE_CYCLES: u64 = 20_000_000;

/// Returns the balance of the given bitcoin address.
///
/// Relies on the `bitcoin_get_balance` endpoint.
/// See https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-bitcoin_get_balance
pub async fn get_balance(network: BitcoinNetwork, address: String) -> BitcoinApiResult<Satoshi> {
    call_with_payment::<(GetBalanceRequest,), (Satoshi,)>(
        Principal::management_canister(),
        "bitcoin_get_balance",
        (GetBalanceRequest {
            address,
            network,
            min_confirmations: None,
        },),
        GET_BALANCE_COST_CYCLES,
    )
    .await
    .map_err(|e| BitcoinApiError::NoBalanceReturned(format!("{:?}", e)))
    .map(|res| res.0)
}

/// Returns the UTXOs of the given bitcoin address.
///
/// NOTE: Relies on the `bitcoin_get_utxos` endpoint.
/// See https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-bitcoin_get_utxos
pub async fn get_utxos(
    network: BitcoinNetwork,
    address: String,
) -> BitcoinApiResult<GetUtxosResponse> {
    call_with_payment::<(GetUtxosRequest,), (GetUtxosResponse,)>(
        Principal::management_canister(),
        "bitcoin_get_utxos",
        (GetUtxosRequest {
            address,
            network,
            filter: None,
        },),
        GET_UTXOS_COST_CYCLES,
    )
    .await
    .map_err(|e| BitcoinApiError::NoUtxosReturned(format!("{:?}", e)))
    .map(|res| res.0)
}

/// Returns the 100 fee percentiles measured in millisatoshi/byte.
/// Percentiles are computed from the last 10,000 transactions (if available).
///
/// Relies on the `bitcoin_get_current_fee_percentiles` endpoint.
/// See https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-bitcoin_get_current_fee_percentiles
pub async fn get_current_fee_percentiles(
    network: BitcoinNetwork,
) -> BitcoinApiResult<Vec<MillisatoshiPerByte>> {
    call_with_payment::<(GetCurrentFeePercentilesRequest,), (Vec<MillisatoshiPerByte>,)>(
        Principal::management_canister(),
        "bitcoin_get_current_fee_percentiles",
        (GetCurrentFeePercentilesRequest { network },),
        GET_CURRENT_FEE_PERCENTILES_CYCLES,
    )
    .await
    .map_err(|e| BitcoinApiError::CurrentFeePercentilesUnavailable(format!("{:?}", e)))
    .map(|res| res.0)
}

/// Sends a (signed) transaction to the bitcoin network.
///
/// Relies on the `bitcoin_send_transaction` endpoint.
/// See https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-bitcoin_send_transaction
pub async fn send_transaction(
    network: BitcoinNetwork,
    transaction: Vec<u8>,
) -> BitcoinApiResult<()> {
    let transaction_fee = SEND_TRANSACTION_BASE_CYCLES
        + (transaction.len() as u64) * SEND_TRANSACTION_PER_BYTE_CYCLES;

    call_with_payment::<(SendTransactionRequest,), ()>(
        Principal::management_canister(),
        "bitcoin_send_transaction",
        (SendTransactionRequest {
            network,
            transaction,
        },),
        transaction_fee,
    )
    .await
    .map_err(|e| BitcoinApiError::TransactionNotSent(format!("{:?}", e)))
}
