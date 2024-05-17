use std::cell::RefCell;

use bitcoin::Transaction;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{
    BitcoinNetwork, GetUtxosResponse, Utxo,
};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use inscriber::constant::UTXO_MIN_CONFIRMATION;
use inscriber::interface::bitcoin_api;
use ord_rs::{Brc20, OrdParser};
use serde::de::DeserializeOwned;

use crate::constant::{
    HTTP_OUTCALL_MAX_RESPONSE_BYTES, HTTP_OUTCALL_PER_CALL_COST, HTTP_OUTCALL_REQ_PER_BYTE_COST,
    HTTP_OUTCALL_RES_DEFAULT_SIZE, HTTP_OUTCALL_RES_PER_BYTE_COST,
};
use crate::interface::bridge_api::{BridgeError, DepositError};
use crate::interface::store::{Brc20Id, Brc20Token, StorableBrc20};
use crate::interface::{get_deposit_address, Brc20TokenResponse, TokenInfo, TransactionHtml};
use crate::state::State;

/// WIP: https://infinityswap.atlassian.net/browse/EPROD-858
///
/// Retrieves and validates the details of a BRC20 token given its ticker.
pub async fn fetch_brc20_token_details(
    state: &RefCell<State>,
    tick: &str,
) -> anyhow::Result<TokenInfo> {
    let indexer_url = { state.borrow().brc20_indexer_url() };

    let token_info = match get_brc20_token_by_ticker(&indexer_url, tick)
        .await
        .map_err(|e| BridgeError::FetchBrc20TokenDetails(e.to_string()))?
    {
        Some(payload) => payload,
        None => {
            return Err(
                BridgeError::FetchBrc20TokenDetails("No BRC20 token found".to_string()).into(),
            )
        }
    };

    // TODO: Add more robust validation checks
    let ticker = token_info.clone().ticker;
    if ticker != tick {
        log::error!("Token tick mismatch. Given: {tick}. Expected: {ticker}");
        return Err(
            BridgeError::FetchBrc20TokenDetails("Incorrect token details".to_string()).into(),
        );
    }

    Ok(token_info)
}

/// Retrieves (and re-constructs) the BRC20 transfer transaction by its ID.
pub(crate) async fn fetch_transfer_transaction(
    state: &RefCell<State>,
    txid: &str,
) -> anyhow::Result<Transaction> {
    let (ic_btc_network, derivation_path, indexer_url) = {
        let state = state.borrow();
        (
            state.ic_btc_network(),
            state.derivation_path(None),
            state.general_indexer_url(),
        )
    };
    let transaction = get_transaction_by_id(&indexer_url, txid).await?;

    let txid_bytes = hex::decode(txid).map_err(|err| {
        log::error!("Failed to decode transaction ID {txid}: {err:?}");
        BridgeError::GetTransactionById("Invalid transaction ID format.".to_string())
    })?;
    let bridge_addr = get_deposit_address(ic_btc_network, derivation_path).await;

    // Validate UTXOs associated with the transaction ID
    let matching_utxos = find_inscription_utxos(ic_btc_network, bridge_addr, txid_bytes).await?;

    if matching_utxos.is_empty() {
        log::warn!("Given transaction ID does not match any of the UTXOs txid.");
        return Err(BridgeError::GetTransactionById(
            "Transaction ID mismatch between the retrieved transaction and UTXOs.".to_string(),
        )
        .into());
    }

    Ok(transaction)
}

/// Parses valid BRC20 inscriptions from the given transaction.
///
/// NOTE:
/// The actual inscription is contained in the reveal transaction, not the eventual transfer.
/// Therefore, we need the ID of the previous output to get the actual BRC20 inscription.
pub(crate) async fn parse_and_validate_inscriptions(
    indexer_url: &str,
    tx: Transaction,
) -> Result<Vec<StorableBrc20>, BridgeError> {
    let reveal_txid = tx
        .input
        .first()
        .map(|input| hex::encode(input.previous_output.txid))
        .ok_or_else(|| BridgeError::GetTransactionById("No inputs in transaction".to_string()))?;

    let reveal_tx = get_transaction_by_id(indexer_url, &reveal_txid)
        .await
        .map_err(|err| BridgeError::GetTransactionById(err.to_string()))?;

    // parse from the actual inscription's reveal transaction
    let parsed_data = OrdParser::parse_all(&reveal_tx)
        .map_err(|e| BridgeError::InscriptionParsing(e.to_string()))?;

    parsed_data.iter().try_fold(
        Vec::new(),
        |mut acc, (token_id, inscription)| match inscription {
            OrdParser::Brc20(brc20) => {
                acc.push(StorableBrc20 {
                    token_id: Brc20Id(*token_id),
                    token: Brc20Token(brc20.clone()),
                });
                Ok(acc)
            }
            _ => Err(BridgeError::InscriptionParsing(
                "Non-BRC20 inscription found".to_string(),
            )),
        },
    )
}

pub(crate) fn get_brc20_data(inscription: &Brc20) -> (u64, &str) {
    match inscription {
        Brc20::Deploy(deploy_func) => (deploy_func.max, &deploy_func.tick),
        Brc20::Mint(mint_func) => (mint_func.amt, &mint_func.tick),
        Brc20::Transfer(transfer_func) => (transfer_func.amt, &transfer_func.tick),
    }
}

async fn get_brc20_token_by_ticker(
    base_indexer_url: &str,
    ticker: &str,
) -> anyhow::Result<Option<TokenInfo>> {
    let payload = http_get_req::<Brc20TokenResponse>(&format!(
        "{base_indexer_url}/ordinals/v1/brc-20/tokens/{ticker}"
    ))
    .await
    .map_err(|err| {
        log::error!("Failed to retrieve BRC20 token details from the indexer: {err:?}");
        BridgeError::FetchBrc20TokenDetails(format!("{err:?}"))
    })?;

    match payload {
        Some(data) => Ok(data.token),
        None => Err(BridgeError::FetchBrc20TokenDetails("Nothing found".to_string()).into()),
    }
}

async fn http_get_req<T>(url: &str) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    let request_params = CanisterHttpRequestArgument {
        url: url.to_owned(),
        max_response_bytes: Some(HTTP_OUTCALL_MAX_RESPONSE_BYTES),
        method: HttpMethod::GET,
        headers: vec![HttpHeader {
            name: "Accept".to_string(),
            value: "application/json".to_string(),
        }],
        body: None,
        transform: None,
    };

    let cycles = get_estimated_http_outcall_cycles(&request_params);

    let (response,) = http_request(request_params, cycles)
        .await
        .map_err(|(_rejection_code, cause)| cause)?;

    log::info!(
        "Indexer responded with: STATUS: {} HEADERS: {:?} BODY: {}",
        response.status,
        response.headers,
        String::from_utf8_lossy(&response.body)
    );

    if response.status == 200u16 {
        let payload = serde_json::from_slice::<T>(&response.body).map_err(|x| x.to_string())?;
        Ok(Some(payload))
    } else if response.status == 404u16 {
        log::info!("No resource found at {url}");
        Ok(None)
    } else {
        Err(BridgeError::BadRequest.to_string())
    }
}

fn get_estimated_http_outcall_cycles(req: &CanisterHttpRequestArgument) -> u128 {
    let headers_size = req.headers.iter().fold(0u128, |len, header| {
        len + header.value.len() as u128 + header.name.len() as u128
    });

    let mut request_size = req.url.len() as u128 + headers_size;

    if let Some(transform) = &req.transform {
        request_size += transform.context.len() as u128;
    }

    if let Some(body) = &req.body {
        request_size += body.len() as u128;
    }

    let http_outcall_cost: u128 = HTTP_OUTCALL_PER_CALL_COST
        + HTTP_OUTCALL_REQ_PER_BYTE_COST * request_size
        + HTTP_OUTCALL_RES_PER_BYTE_COST
            * req
                .max_response_bytes
                .unwrap_or(HTTP_OUTCALL_RES_DEFAULT_SIZE) as u128;

    http_outcall_cost
}

async fn get_transaction_by_id(indexer_url: &str, tx_id: &str) -> anyhow::Result<Transaction> {
    let transaction = http_get_req::<TransactionHtml>(&format!("{indexer_url}/tx/{tx_id}"))
        .await
        .map_err(|err| {
            log::error!("Failed to retrieve transaction from the indexer: {err:?}");
            BridgeError::GetTransactionById(format!("{err:?}"))
        })?
        .ok_or_else(|| BridgeError::GetTransactionById("Transaction not found.".to_string()))?
        .transaction;

    Ok(transaction)
}

/// Validates a reveal transaction ID by checking if it matches
/// the transaction IDs of the received UTXOs and returns all matching UTXOs.
///
/// TODO: reduce latency by using derivation_path
async fn find_inscription_utxos(
    network: BitcoinNetwork,
    address: String,
    txid: Vec<u8>,
) -> Result<Vec<Utxo>, BridgeError> {
    let utxo_response = bitcoin_api::get_utxos(network, address)
        .await
        .map_err(|e| BridgeError::FindInscriptionUtxos(e.to_string()))?;

    let validated_utxos = validate_utxos(utxo_response)
        .map_err(|err| BridgeError::FindInscriptionUtxos(format!("{err:?}")))?;

    let matching_utxos = validated_utxos
        .into_iter()
        .filter(|utxo| reverse_txid_byte_order(&utxo.outpoint.txid) == txid)
        .collect::<Vec<Utxo>>();

    if matching_utxos.is_empty() {
        let tx_id = hex::encode(txid);
        log::info!("No matching UTXOs found for transaction ID {tx_id}");
        Err(BridgeError::FindInscriptionUtxos(
            "No UTXOs found".to_string(),
        ))
    } else {
        Ok(matching_utxos)
    }
}

fn validate_utxos(utxo_response: GetUtxosResponse) -> Result<Vec<Utxo>, DepositError> {
    let min_confirmations = UTXO_MIN_CONFIRMATION;
    let current_confirmations = utxo_response
        .utxos
        .iter()
        .map(|utxo| utxo_response.tip_height - utxo.height + 1)
        .min()
        .unwrap_or_default();

    if min_confirmations > current_confirmations {
        Err(DepositError::Pending {
            min_confirmations,
            current_confirmations,
        })
    } else {
        Ok(utxo_response.utxos)
    }
}

/// Reverses the byte order of the transaction ID.
///
/// The IC management canister returns bytes of txid in reversed order,
/// so we need to undo the operation first before before consuming the output.
fn reverse_txid_byte_order(tx_id: &[u8]) -> Vec<u8> {
    tx_id.iter().copied().rev().collect::<Vec<u8>>()
}
