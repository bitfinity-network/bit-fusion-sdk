use candid::Principal;
use ic_exports::ic_cdk::api::call::call_with_payment;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{
    BitcoinNetwork, GetBalanceRequest, GetCurrentFeePercentilesRequest, GetUtxosRequest,
    GetUtxosResponse, MillisatoshiPerByte, Satoshi, SendTransactionRequest, Utxo, UtxoFilter,
};

// The fees for the various bitcoin endpoints.
const GET_BALANCE_COST_CYCLES: u64 = 100_000_000;
const GET_UTXOS_COST_CYCLES: u64 = 10_000_000_000;
const GET_CURRENT_FEE_PERCENTILES_CYCLES: u64 = 100_000_000;
const SEND_TRANSACTION_BASE_CYCLES: u64 = 5_000_000_000;
const SEND_TRANSACTION_PER_BYTE_CYCLES: u64 = 20_000_000;

// There is an upper bound of 144 on the minimum number of confirmations.
// If a larger minimum number of confirmations is specified, the call is rejected.
// In practice, this value is set around 6.
//
// Reference: https://internetcomputer.org/docs/current/references/ic-interface-spec#ic-bitcoin_get_utxos
#[allow(unused)]
const MIN_CONFIRMATIONS: u32 = 6;

/// Returns the balance of the given bitcoin address.
///
/// Relies on the `bitcoin_get_balance` endpoint.
/// See https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-bitcoin_get_balance
pub async fn get_balance(network: BitcoinNetwork, address: String) -> u64 {
    let balance_res: Result<(Satoshi,), _> = call_with_payment(
        Principal::management_canister(),
        "bitcoin_get_balance",
        (GetBalanceRequest {
            address,
            network,
            min_confirmations: None,
        },),
        GET_BALANCE_COST_CYCLES,
    )
    .await;

    balance_res
        .expect("Failed to retrieve balance for specified address")
        .0
}

/// Fetches all UTXOs for the given address using pagination.
///
/// Returns a vector of all UTXOs for the given Bitcoin address.
/// NOTE: Relies on the `bitcoin_get_utxos` endpoint.
/// See https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-bitcoin_get_utxos
pub async fn get_utxos(
    network: BitcoinNetwork,
    address: String,
) -> Result<GetUtxosResponse, String> {
    let mut all_utxos = Vec::<Utxo>::new();
    let mut page_filter: Option<UtxoFilter> = None;
    let mut tip_block_hash = Vec::<u8>::new();
    let mut tip_height = 0u32;

    let mut last_page: Option<Vec<u8>>;

    loop {
        let get_utxos_request = GetUtxosRequest {
            address: address.clone(),
            network,
            filter: page_filter,
        };

        let utxos_res: Result<(GetUtxosResponse,), _> = call_with_payment(
            Principal::management_canister(),
            "bitcoin_get_utxos",
            (get_utxos_request,),
            GET_UTXOS_COST_CYCLES,
        )
        .await;

        match utxos_res {
            Ok(response) => {
                let (get_utxos_response,) = response;
                all_utxos.extend(get_utxos_response.utxos);
                // Update tip_block_hash and tip_height only if they are not already set
                if tip_block_hash.is_empty() {
                    tip_block_hash = get_utxos_response.tip_block_hash;
                }
                if tip_height == 0 {
                    tip_height = get_utxos_response.tip_height;
                }
                last_page = get_utxos_response.next_page.clone();
                page_filter = last_page.clone().map(UtxoFilter::Page);
            }
            Err(e) => return Err(format!("{:?}", e)),
        }

        if page_filter.is_none() {
            break;
        }
    }

    Ok(GetUtxosResponse {
        utxos: all_utxos,
        tip_block_hash,
        tip_height,
        next_page: last_page,
    })
}

/// Returns the 100 fee percentiles measured in millisatoshi/byte.
/// Percentiles are computed from the last 10,000 transactions (if available).
///
/// Relies on the `bitcoin_get_current_fee_percentiles` endpoint.
/// See https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-bitcoin_get_current_fee_percentiles
pub async fn get_current_fee_percentiles(network: BitcoinNetwork) -> Vec<MillisatoshiPerByte> {
    let res: Result<(Vec<MillisatoshiPerByte>,), _> = call_with_payment(
        Principal::management_canister(),
        "bitcoin_get_current_fee_percentiles",
        (GetCurrentFeePercentilesRequest { network },),
        GET_CURRENT_FEE_PERCENTILES_CYCLES,
    )
    .await;

    res.unwrap().0
}

/// Sends a (signed) transaction to the bitcoin network.
///
/// Relies on the `bitcoin_send_transaction` endpoint.
/// See https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-bitcoin_send_transaction
pub async fn send_transaction(network: BitcoinNetwork, transaction: Vec<u8>) {
    let transaction_fee = SEND_TRANSACTION_BASE_CYCLES
        + (transaction.len() as u64) * SEND_TRANSACTION_PER_BYTE_CYCLES;

    let res: Result<(), _> = call_with_payment(
        Principal::management_canister(),
        "bitcoin_send_transaction",
        (SendTransactionRequest {
            network,
            transaction,
        },),
        transaction_fee,
    )
    .await;

    res.unwrap();
}
