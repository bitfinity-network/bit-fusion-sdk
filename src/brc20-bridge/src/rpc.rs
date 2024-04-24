use std::cell::RefCell;
use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use inscriber::interface::bitcoin_api;
use ord_rs::{Brc20, OrdParser};
use serde::Deserialize;

use crate::constant::{BRC20_TICKER_LEN, CYCLES_PER_HTTP_REQUEST, MAX_HTTP_RESPONSE_BYTES};
use crate::interface::bridge_api::{BridgeError, Erc20MintError};
use crate::interface::store::Brc20TokenInfo;
use crate::state::{RegtestRpcConfig, State};

/// Retrieves and validates the details of a BRC20 token given its ticker.
pub(crate) async fn get_brc20_token_info(
    state: &RefCell<State>,
    ticker: String,
    holder: String,
) -> Result<Brc20TokenInfo, BridgeError> {
    let network = state.borrow().ic_btc_network();
    let brc20_info = match network {
        BitcoinNetwork::Regtest => state.borrow().brc20_token_info(),
        _ => crate::rpc::fetch_brc20_token_details(state, ticker, holder)
            .await
            .map_err(|e| Erc20MintError::Brc20Bridge(e.to_string()))?,
    };
    Ok(brc20_info)
}

async fn fetch_brc20_token_details(
    state: &RefCell<State>,
    tick: String,
    holder: String,
) -> anyhow::Result<Brc20TokenInfo> {
    let (network, indexer_url) = {
        let state = state.borrow();
        (state.btc_network(), state.ordinals_indexer_url())
    };

    // check that BTC address is valid and/or
    // corresponds to the network.
    is_valid_btc_address(&holder, network)?;

    let url = format!("{indexer_url}/{tick}");

    log::info!("Retrieving {tick} token details from: {url}");

    let request_params = CanisterHttpRequestArgument {
        url,
        max_response_bytes: Some(MAX_HTTP_RESPONSE_BYTES),
        method: HttpMethod::GET,
        headers: vec![HttpHeader {
            name: "Accept".to_string(),
            value: "application/json".to_string(),
        }],
        body: None,
        transform: None,
    };

    let result = http_request(request_params, CYCLES_PER_HTTP_REQUEST)
        .await
        .map_err(|err| BridgeError::FetchBrc20TokenDetails(format!("{err:?}")))?
        .0;

    if result.status.to_string() != "200" {
        log::error!("Failed to fetch data: HTTP status {}", result.status);
        return Err(BridgeError::FetchBrc20TokenDetails("Failed to fetch data".to_string()).into());
    }

    log::info!(
        "Response from indexer: Status: {} Body: {}",
        result.status,
        String::from_utf8_lossy(&result.body)
    );

    let token_details: Brc20TokenResponse =
        serde_json::from_slice(&result.body).map_err(|err| {
            log::error!("Failed to retrieve {tick} token details from the indexer: {err:?}");
            BridgeError::FetchBrc20TokenDetails(format!("{err:?}"))
        })?;

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
    let (network, indexer_url) = {
        let state = state.borrow();
        (state.btc_network(), state.general_indexer_url())
    };

    let network_str = network_as_str(network);
    let url = format!("{indexer_url}{network_str}/api/tx/{reveal_tx_id}");

    let request_params = CanisterHttpRequestArgument {
        url,
        max_response_bytes: Some(MAX_HTTP_RESPONSE_BYTES),
        method: HttpMethod::GET,
        headers: vec![HttpHeader {
            name: "Accept".to_string(),
            value: "application/json".to_string(),
        }],
        body: None,
        transform: None,
    };

    let result = http_request(request_params, CYCLES_PER_HTTP_REQUEST)
        .await
        .map_err(|err| BridgeError::GetTransactionById(format!("{err:?}")))?
        .0;

    if result.status.to_string() != "200" {
        log::error!("Failed to fetch data: HTTP status {}", result.status);
        return Err(BridgeError::FetchBrc20TokenDetails("Failed to fetch data".to_string()).into());
    }

    log::info!(
        "Response from indexer: Status: {} Body: {}",
        result.status,
        String::from_utf8_lossy(&result.body)
    );

    let tx: TxInfo = serde_json::from_slice(&result.body).map_err(|err| {
        log::error!("Failed to retrieve the reveal transaction from the indexer: {err:?}");
        BridgeError::GetTransactionById(format!("{err:?}"))
    })?;

    tx.try_into()
}

/// Retrieves (and re-constructs) the reveal transaction by its ID.
///
/// We use the reveal transaction (as opposed to the commit transaction)
/// because it contains the actual BRC20 inscription that needs to be parsed.
pub(crate) async fn fetch_reveal_transaction_regtest(
    state: &RefCell<State>,
    bridge_addr: String,
    reveal_tx_id: &str,
) -> anyhow::Result<Transaction> {
    use bitcoin::consensus::encode::deserialize;

    let network = state.borrow().ic_btc_network();

    let RegtestRpcConfig {
        url,
        user,
        password,
    } = state.borrow().regtest_rpc_config();

    let rpc_client = Client::new(&url, Auth::UserPass(user, password))
        .map_err(|e| BridgeError::RegtestRpc(format!("Failed to connect to {:?}", e)))?;

    let brc20_utxo = find_inscription_utxo(network, bridge_addr, reveal_tx_id.as_bytes().to_vec())
        .await
        .map_err(|e| BridgeError::GetTransactionById(e.to_string()))?;

    let txid =
        Txid::from_slice(&brc20_utxo.outpoint.txid).expect("Failed to parse Txid from slice");

    let raw_tx = rpc_client
        .get_raw_transaction_hex(&txid, None)
        .map_err(|e| BridgeError::RegtestRpc(e.to_string()))?;

    Ok(deserialize(raw_tx.as_bytes()).map_err(|e| BridgeError::RegtestRpc(e.to_string()))?)
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

async fn find_inscription_utxo(
    network: BitcoinNetwork,
    addr: String,
    txid: Vec<u8>,
) -> Result<Utxo, BridgeError> {
    let utxos = bitcoin_api::get_utxos(network, addr)
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
