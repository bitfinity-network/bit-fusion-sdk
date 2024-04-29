use std::cell::RefCell;
use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use futures::TryFutureExt;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpMethod, HttpResponse,
};
use ic_exports::ic_kit::ic;
use inscriber::interface::bitcoin_api;
use ord_rs::{Brc20, OrdParser};
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::constant::{
    BRC20_TICKER_LEN, HTTP_OUTCALL_PER_CALL_COST, HTTP_OUTCALL_REQ_PER_BYTE_COST,
    HTTP_OUTCALL_RES_DEFAULT_SIZE, HTTP_OUTCALL_RES_PER_BYTE_COST,
};
use crate::interface::bridge_api::BridgeError;
use crate::interface::get_deposit_address;
use crate::interface::store::Brc20TokenInfo;
use crate::state::State;

/// Retrieves and validates the details of a BRC20 token given its ticker.
pub(crate) async fn fetch_brc20_token_details(
    state: &RefCell<State>,
    tick: String,
    holder: String,
) -> anyhow::Result<Brc20TokenInfo> {
    let (network, indexer_url) = {
        let state = state.borrow();
        (state.btc_network(), state.indexer_url())
    };

    // check that BTC address is valid and/or
    // corresponds to the network.
    is_valid_btc_address(&holder, network)?;

    let token_details = match get_brc20_token_by_ticker(state, &indexer_url, &tick)
        .await
        .map_err(|e| BridgeError::FetchBrc20TokenDetails(e.to_string()))?
    {
        Some(token_res) => token_res,
        None => {
            return Err(
                BridgeError::FetchBrc20TokenDetails("No BRC20 token found".to_owned()).into(),
            )
        }
    };

    let TokenInfo {
        tx_id,
        address,
        ticker,
        ..
    } = token_details.token;

    if address != holder && tick != ticker {
        log::error!(
            "Token details mismatch. Given: {:?}. Expected: {:?}",
            (tick, holder),
            (ticker, address)
        );

        return Err(
            BridgeError::FetchBrc20TokenDetails("Incorrect token details".to_string()).into(),
        );
    }

    Ok(Brc20TokenInfo {
        ticker,
        holder: address,
        tx_id,
    })
}

/// Retrieves (and re-constructs) the reveal transaction by its ID.
///
/// We use the reveal transaction (as opposed to the commit transaction)
/// because it contains the actual BRC20 inscription that needs to be parsed.
pub(crate) async fn fetch_reveal_transaction(
    state: &RefCell<State>,
    reveal_tx_id: &str,
) -> anyhow::Result<Transaction> {
    let (ic_btc_network, indexer_url, derivation_path) = {
        let state = state.borrow();
        (
            state.ic_btc_network(),
            state.indexer_url(),
            state.derivation_path(None),
        )
    };

    let bridge_addr = get_deposit_address(ic_btc_network, derivation_path).await;

    let brc20_utxo = find_inscription_utxo(
        ic_btc_network,
        bridge_addr,
        reveal_tx_id.as_bytes().to_vec(),
    )
    .map_err(|e| BridgeError::FindInscriptionUtxo(e.to_string()))
    .await?;

    let txid = hex::encode(brc20_utxo.outpoint.txid);

    let transaction = match get_brc20_transaction_by_id(state, &indexer_url, &txid)
        .await
        .map_err(|e| BridgeError::GetTransactionById(e.to_string()))?
    {
        Some(token_res) => token_res,
        None => {
            return Err(BridgeError::GetTransactionById("No BRC20 token found".to_owned()).into())
        }
    };

    Ok(transaction)
}

pub(crate) async fn parse_and_validate_inscription(
    reveal_tx: Transaction,
) -> Result<Brc20, BridgeError> {
    log::info!("Parsing BRC20 inscription from transaction");

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

async fn get_brc20_token_by_ticker(
    state: &RefCell<State>,
    base_indexer_url: &str,
    ticker: &str,
) -> Result<Option<Brc20TokenResponse>, String> {
    let network = { state.borrow().btc_network() };

    match network {
        Network::Regtest => {
            http_get_req_mock::<Brc20TokenResponse>(&format!(
                "{base_indexer_url}/ordinals/v1/brc-20/tokens/{ticker}"
            ))
            .await
        }
        _ => {
            http_get_req::<Brc20TokenResponse>(&format!(
                "{base_indexer_url}/ordinals/v1/brc-20/tokens/{ticker}"
            ))
            .await
        }
    }
}

async fn get_brc20_transaction_by_id(
    state: &RefCell<State>,
    base_indexer_url: &str,
    txid: &str,
) -> Result<Option<Transaction>, String> {
    let network = { state.borrow().btc_network() };
    let btc_network_str = network_as_str(network);

    match network {
        Network::Regtest => {
            http_get_req_mock::<Transaction>(&format!(
                "{base_indexer_url}{btc_network_str}/api/tx/{txid}"
            ))
            .await
        }
        _ => {
            http_get_req::<Transaction>(&format!(
                "{base_indexer_url}{btc_network_str}/api/tx/{txid}"
            ))
            .await
        }
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

async fn http_get_req<T>(url: &str) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    let req = CanisterHttpRequestArgument {
        url: url.to_string(),
        max_response_bytes: None,
        method: HttpMethod::GET,
        headers: Vec::new(),
        body: None,
        transform: None,
    };

    let cycles = get_estimated_http_outcall_cycles(&req);

    let (resp,) = http_request(req, cycles)
        .await
        .map_err(|(_rejection_code, cause)| cause)?;

    if resp.status == 200u16 {
        let data = serde_json::from_slice(&resp.body).map_err(|x| x.to_string())?;

        Ok(Some(data))
    } else if resp.status == 404u16 {
        Ok(None)
    } else {
        Err("Invalid http status code".to_string())
    }
}

async fn http_get_req_mock<T>(url: &str) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    let canister_id = ic::id();
    let (maybe_resp,): (Option<HttpResponse>,) =
        ic::call(canister_id, "get_http_mock", (url.to_string(),))
            .await
            .map_err(|(_rejection_code, cause)| cause)?;

    let Some(resp) = maybe_resp else {
        panic!("HTTP mock is not found")
    };

    if resp.status == 200u16 {
        let data = serde_json::from_slice(&resp.body).map_err(|x| x.to_string())?;

        Ok(Some(data))
    } else if resp.status == 404u16 {
        Ok(None)
    } else {
        Err("Invalid http status code".to_string())
    }
}

fn network_as_str(network: Network) -> &'static str {
    match network {
        Network::Testnet => "/testnet",
        Network::Regtest => "/regtest",
        Network::Signet => "/signet",
        _ => "",
    }
}

fn is_valid_btc_address(addr: &str, network: Network) -> Result<bool, BridgeError> {
    let network_str = network_as_str(network);

    if !Address::from_str(addr)
        .expect("Failed to convert to bitcoin address")
        .is_valid_for_network(network)
    {
        log::error!("The given bitcoin address {addr} is not valid for {network_str}");
        return Err(BridgeError::MalformedAddress(addr.to_string()));
    }

    Ok(true)
}

/// Validates a reveal transaction ID by checking if it matches
/// the transaction ID of the received UTXO.
///
/// TODO: 1. reduce latency by using derivation_path
///       2. filter out pending UTXOs
async fn find_inscription_utxo(
    network: BitcoinNetwork,
    deposit_addr: String,
    txid: Vec<u8>,
) -> Result<Utxo, BridgeError> {
    let utxos = bitcoin_api::get_utxos(network, deposit_addr)
        .await
        .map_err(|e| BridgeError::GetTransactionById(e.to_string()))?
        .utxos;

    let brc20_utxo = utxos
        .iter()
        .find(|utxo| utxo.outpoint.txid == txid)
        .cloned();

    match brc20_utxo {
        Some(utxo) => Ok(utxo),
        None => Err(BridgeError::GetTransactionById(
            "No matching UTXO found".to_string(),
        )),
    }
}

#[allow(unused)]
fn validate_utxos(
    _network: BitcoinNetwork,
    _addr: &str,
    _utxos: &[Utxo],
) -> Result<Vec<Utxo>, String> {
    todo!()
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Brc20TokenResponse {
    token: TokenInfo,
    supply: TokenSupply,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TokenInfo {
    id: String,
    number: u32,
    block_height: u32,
    tx_id: String,
    address: String,
    ticker: String,
    max_supply: String,
    mint_limit: String,
    decimals: u8,
    deploy_timestamp: u64,
    minted_supply: String,
    tx_count: u32,
    self_mint: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TokenSupply {
    max_supply: String,
    minted_supply: String,
    holders: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TxInfo {
    version: i32,
    locktime: u32,
    vin: Vec<Vin>,
    vout: Vec<Vout>,
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
struct Vin {
    txid: String,
    vout: u32,
    sequence: u32,
    is_coinbase: bool,
    prevout: Prevout,
    witness: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Prevout {
    scriptpubkey: String,
    scriptpubkey_asm: String,
    scriptpubkey_type: String,
    scriptpubkey_address: Option<String>,
    value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Vout {
    scriptpubkey: String,
    scriptpubkey_asm: String,
    scriptpubkey_type: String,
    scriptpubkey_address: Option<String>,
    value: u64,
}
