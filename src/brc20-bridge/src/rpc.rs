use std::cell::RefCell;
use std::str::FromStr;

use bitcoin::{Address, Network, Transaction, Txid};
use clap::ValueEnum;
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
use ordinals::SpacedRune;
use serde::{Deserialize, Serialize};

use crate::constant::{
    HTTP_OUTCALL_MAX_RESPONSE_BYTES, HTTP_OUTCALL_PER_CALL_COST, HTTP_OUTCALL_REQ_PER_BYTE_COST,
    HTTP_OUTCALL_RES_DEFAULT_SIZE, HTTP_OUTCALL_RES_PER_BYTE_COST,
};
use crate::interface::bridge_api::{BridgeError, DepositError};
use crate::interface::get_deposit_address;
use crate::interface::store::Brc20Token;
use crate::state::State;

/// Retrieves and validates the details of a BRC20 token given its ticker.
pub async fn fetch_brc20_token_details(
    state: &RefCell<State>,
    tick: String,
    holder: String,
) -> anyhow::Result<Brc20Token> {
    let (network, indexer_url) = {
        let state = state.borrow();
        (state.btc_network(), state.indexer_url())
    };

    // check that BTC address is valid and/or
    // corresponds to the network.
    is_valid_btc_address(&holder, network)?;

    let TokenInfo {
        tx_id,
        address,
        ticker,
        ..
    } = match get_brc20_token_by_ticker(&indexer_url, &tick)
        .await
        .map_err(|e| BridgeError::FetchBrc20TokenDetails(e.to_string()))?
    {
        Some(token) => token,
        None => {
            return Err(
                BridgeError::FetchBrc20TokenDetails("No BRC20 token found".to_string()).into(),
            )
        }
    };

    if address != holder && ticker != tick {
        log::error!(
            "Token details mismatch. Given: {:?}. Expected: {:?}",
            (tick, holder),
            (ticker, address)
        );
        return Err(
            BridgeError::FetchBrc20TokenDetails("Incorrect token details".to_string()).into(),
        );
    }

    Ok(Brc20Token {
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

pub(crate) fn parse_and_validate_inscriptions(
    reveal_tx: Transaction,
) -> Result<Brc20, BridgeError> {
    let inscriptions = OrdParser::parse_all(&reveal_tx)
        .map_err(|e| BridgeError::InscriptionParsing(e.to_string()))?
        .into_iter()
        .map(|parsed| match parsed {
            OrdParser::Brc20(payload) => Ok(payload),
            _ => Err(BridgeError::InscriptionParsing(
                "Not a valid BRC20".to_string(),
            )),
        })
        .collect::<Result<Vec<Brc20>, BridgeError>>();

    match inscriptions {
        Ok(_brc20s) => todo!(),
        Err(err) => Err(err),
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
    base_indexer_url: &str,
    ticker: &str,
) -> anyhow::Result<Option<TokenInfo>> {
    // let url = format!("{base_indexer_url}/ordinals/v1/brc-20/tokens/{ticker}");
    let url = format!("{base_indexer_url}/brc-20/tokens/{ticker}");
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
        "Indexer responded with: STATUS: {} HEADERS: {:?} BODY: {}",
        response.status,
        response.headers,
        String::from_utf8_lossy(&response.body)
    );

    let brc20_resp: Brc20TokenResponse = serde_json::from_slice(&response.body).map_err(|err| {
        log::error!("Failed to retrieve the reveal transaction from the indexer: {err:?}");
        BridgeError::FetchBrc20TokenDetails(format!("{err:?}"))
    })?;

    Ok(brc20_resp.token)
}

async fn get_brc20_transaction_by_id(
    base_indexer_url: &str,
    txid: &str,
    _network_str: &str,
) -> anyhow::Result<Transaction> {
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
        "Indexer responded with: STATUS: {} HEADERS: {:?} BODY: {}",
        response.status,
        response.headers,
        String::from_utf8_lossy(&response.body)
    );

    let tx_html: TransactionHtml = serde_json::from_slice(&response.body).map_err(|err| {
        log::error!("Failed to retrieve the reveal transaction from the indexer: {err:?}");
        BridgeError::GetTransactionById(format!("{err:?}"))
    })?;

    Ok(tx_html.transaction)
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TransactionHtml {
    chain: Chain,
    etching: Option<SpacedRune>,
    inscription_count: u32,
    transaction: Transaction,
    txid: Txid,
}

// To avoid pulling the entire `ord` crate into our dependencies, the following code is
// copied from https://github.com/ordinals/ord/blob/master/src/chain.rs

#[derive(Default, ValueEnum, Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Chain {
    #[default]
    #[value(alias("main"))]
    Mainnet,
    #[value(alias("test"))]
    Testnet,
    Signet,
    Regtest,
}

impl From<Chain> for Network {
    fn from(chain: Chain) -> Network {
        match chain {
            Chain::Mainnet => Network::Bitcoin,
            Chain::Testnet => Network::Testnet,
            Chain::Signet => Network::Signet,
            Chain::Regtest => Network::Regtest,
        }
    }
}

impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Mainnet => "mainnet",
                Self::Regtest => "regtest",
                Self::Signet => "signet",
                Self::Testnet => "testnet",
            }
        )
    }
}

impl std::str::FromStr for Chain {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "regtest" => Ok(Self::Regtest),
            "signet" => Ok(Self::Signet),
            "testnet" => Ok(Self::Testnet),
            _ => anyhow::bail!("invalid chain `{s}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Brc20TokenResponse {
    token: Option<TokenInfo>,
    supply: Option<TokenSupply>,
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
