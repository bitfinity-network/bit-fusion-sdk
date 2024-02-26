use candid::Principal;
use ic_cdk::api::call::{call_with_payment, RejectionCode};
use ic_cdk::api::management_canister::bitcoin::{
    BitcoinNetwork, GetBalanceRequest, GetUtxosRequest, GetUtxosResponse, Satoshi,
    SendTransactionRequest,
};

const GET_BALANCE_COST_CYCLES: u64 = 100_000_000;
const GET_UTXOS_COST_CYCLES: u64 = 10_000_000_000;
const SEND_TRANSACTION_BASE_CYCLES: u64 = 5_000_000_000;
const SEND_TRANSACTION_PER_BYTE_CYCLES: u64 = 20_000_000;

#[derive(Debug, Clone)]
pub enum BitcoinApiError {
    GetBalanceRequest(String),
    GetUtxosRequest(String),
    SendTransactionRequest(String),
}

impl From<RejectionCode> for BitcoinApiError {
    fn from(_err: ic_cdk::api::call::RejectionCode) -> Self {
        todo!()
    }
}

pub async fn get_balance(
    network: BitcoinNetwork,
    address: String,
) -> Result<Satoshi, BitcoinApiError> {
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
    .map_err(|e| BitcoinApiError::GetBalanceRequest(format!("Failed to get balance: {:?}", e)))
    .map(|res| res.0)
}

pub async fn get_utxos(
    network: BitcoinNetwork,
    address: String,
) -> Result<GetUtxosResponse, BitcoinApiError> {
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
    .map_err(|e| BitcoinApiError::GetUtxosRequest(format!("Failed to get UTXOs: {:?}", e)))
    .map(|res| res.0)
}

pub async fn send_transaction(
    network: BitcoinNetwork,
    transaction: Vec<u8>,
) -> Result<(), BitcoinApiError> {
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
    .map_err(|e| {
        BitcoinApiError::SendTransactionRequest(format!("Failed to send transaction: {:?}", e))
    })
}
