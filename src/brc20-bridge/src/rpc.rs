use std::cell::RefCell;
use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::transaction::Version;
use bitcoin::{
    Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
};
use futures::TryFutureExt;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{
    BitcoinNetwork, GetUtxosResponse, Utxo,
};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use inscriber::constant::UTXO_MIN_CONFIRMATION;
use inscriber::interface::bitcoin_api;
use ord_rs::{Brc20, OrdParser};
use serde::Deserialize;

use crate::constant::{
    BRC20_TICKER_LEN, HTTP_OUTCALL_MAX_RESPONSE_BYTES, HTTP_OUTCALL_PER_CALL_COST,
    HTTP_OUTCALL_REQ_PER_BYTE_COST, HTTP_OUTCALL_RES_DEFAULT_SIZE, HTTP_OUTCALL_RES_PER_BYTE_COST,
};
use crate::interface::bridge_api::{BridgeError, DepositError};
use crate::interface::get_deposit_address;
use crate::state::State;

/// Retrieves (and re-constructs) the reveal transaction by its ID.
///
/// We use the reveal transaction (as opposed to the commit transaction)
/// because it contains the actual BRC20 inscription that needs to be parsed.
pub(crate) async fn fetch_reveal_transaction(
    state: &RefCell<State>,
    reveal_tx_id: &str,
) -> anyhow::Result<Transaction> {
    let (ic_btc_network, derivation_path, indexer_url, btc_network) = {
        let state = state.borrow();
        (
            state.ic_btc_network(),
            state.derivation_path(None),
            state.indexer_url(),
            network_as_str(state.btc_network()),
        )
    };

    let bridge_addr = get_deposit_address(ic_btc_network, derivation_path).await;

    let tx_id = hex::decode(reveal_tx_id).expect("failed to decode txid to bytes");

    let brc20_utxo = find_inscription_utxo(ic_btc_network, bridge_addr, tx_id)
        .map_err(|e| BridgeError::FindInscriptionUtxo(e.to_string()))
        .await?;

    let tx_id = hex::encode(reverse_txid_byte_order(&brc20_utxo));

    Ok(
        get_brc20_transaction_by_id(&indexer_url, &tx_id, btc_network)
            .map_err(|e| BridgeError::GetTransactionById(e.to_string()))
            .await?,
    )
}

pub(crate) fn parse_and_validate_inscription(reveal_tx: Transaction) -> Result<Brc20, BridgeError> {
    let inscription = OrdParser::parse::<Brc20>(&reveal_tx)
        .map_err(|e| BridgeError::InscriptionParsing(e.to_string()))?;

    match inscription {
        Some(brc20) => {
            let ticker = get_brc20_data(&brc20).1;
            if ticker.len() != BRC20_TICKER_LEN {
                return Err(BridgeError::InscriptionParsing(
                    "BRC20 ticker (symbol) should be only 4 letters".to_string(),
                ));
            }
            log::info!("BRC20 inscription validated");
            Ok(brc20)
        }
        None => Err(BridgeError::InscriptionParsing(
            "No BRC20 inscription associated with this transaction".to_string(),
        )),
    }
}

pub(crate) fn get_brc20_data(inscription: &Brc20) -> (u64, &str) {
    match inscription {
        Brc20::Deploy(deploy_func) => (deploy_func.max, &deploy_func.tick),
        Brc20::Mint(mint_func) => (mint_func.amt, &mint_func.tick),
        Brc20::Transfer(transfer_func) => (transfer_func.amt, &transfer_func.tick),
    }
}

async fn get_brc20_transaction_by_id(
    base_indexer_url: &str,
    txid: &str,
    _network_str: &str,
) -> anyhow::Result<Transaction> {
    // let url = format!("{base_indexer_url}{network_str}/api/tx/{txid}");
    let url = format!("{base_indexer_url}/tx/{txid}");

    let request_params = CanisterHttpRequestArgument {
        url,
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

    let response = http_request(request_params, cycles)
        .await
        .map_err(|err| BridgeError::FetchBrc20TokenDetails(format!("{err:?}")))?
        .0;

    log::info!(
        "Indexer responded with: status: {} body: {}",
        response.status,
        String::from_utf8_lossy(&response.body)
    );

    let tx: TxInfo = serde_json::from_slice(&response.body).map_err(|err| {
        log::error!("Failed to retrieve the reveal transaction from the indexer: {err:?}");
        BridgeError::GetTransactionById(format!("{err:?}"))
    })?;

    tx.try_into()
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

fn network_as_str(network: Network) -> &'static str {
    match network {
        Network::Testnet => "/testnet",
        Network::Regtest => "/regtest",
        Network::Signet => "/signet",
        _ => "",
    }
}

/// Validates a reveal transaction ID by checking if it matches
/// the transaction ID of the received UTXO.
///
/// TODO: 1. reduce latency by using derivation_path
///       2. filter out pending UTXOs
async fn find_inscription_utxo(
    network: BitcoinNetwork,
    address: String,
    txid: Vec<u8>,
) -> Result<Utxo, BridgeError> {
    let utxo_response = bitcoin_api::get_utxos(network, address)
        .await
        .map_err(|e| BridgeError::GetTransactionById(e.to_string()))?;

    let validated_utxos = validate_utxos(utxo_response)
        .map_err(|err| BridgeError::FindInscriptionUtxo(format!("{err:?}")))?;

    match validated_utxos
        .iter()
        .find(|&utxo| reverse_txid_byte_order(utxo) == txid)
        .cloned()
    {
        Some(utxo) => Ok(utxo),
        None => {
            let tx_id = hex::encode(txid);
            log::info!("No matching UTXO found for transaction ID {tx_id}");
            Err(BridgeError::FindInscriptionUtxo(
                "No matching UTXO found".to_string(),
            ))
        }
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
fn reverse_txid_byte_order(utxo: &Utxo) -> Vec<u8> {
    utxo.outpoint
        .txid
        .iter()
        .copied()
        .rev()
        .collect::<Vec<u8>>()
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TxInfo {
    pub version: i32,
    pub locktime: u32,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
}

impl TryFrom<TxInfo> for Transaction {
    type Error = anyhow::Error;

    fn try_from(info: TxInfo) -> Result<Self, Self::Error> {
        let version = Version(info.version);
        let lock_time = LockTime::from_consensus(info.locktime);

        let mut tx_in = Vec::with_capacity(info.vin.len());
        for input in info.vin {
            let txid = Txid::from_str(&input.txid)?;
            let vout = input.vout;
            let script_sig = ScriptBuf::from_hex(&input.prevout.scriptpubkey)?;

            let mut witness = Witness::new();
            for item in input.witness {
                witness.push(ScriptBuf::from_hex(&item)?);
            }

            let tx_input = TxIn {
                previous_output: OutPoint { txid, vout },
                script_sig,
                sequence: Sequence(input.sequence),
                witness,
            };

            tx_in.push(tx_input);
        }

        let mut tx_out = Vec::with_capacity(info.vout.len());
        for output in info.vout {
            let script_pubkey = ScriptBuf::from_hex(&output.scriptpubkey)?;
            let value = Amount::from_sat(output.value);

            tx_out.push(TxOut {
                script_pubkey,
                value,
            });
        }

        Ok(Transaction {
            version,
            lock_time,
            input: tx_in,
            output: tx_out,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Vin {
    pub txid: String,
    pub vout: u32,
    pub sequence: u32,
    pub is_coinbase: bool,
    pub prevout: Prevout,
    pub witness: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Prevout {
    pub scriptpubkey: String,
    pub scriptpubkey_asm: String,
    pub scriptpubkey_type: String,
    pub scriptpubkey_address: Option<String>,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Vout {
    pub scriptpubkey: String,
    pub scriptpubkey_asm: String,
    pub scriptpubkey_type: String,
    pub scriptpubkey_address: Option<String>,
    pub value: u64,
}
