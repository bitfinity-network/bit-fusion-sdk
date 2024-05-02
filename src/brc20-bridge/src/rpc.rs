use std::cell::RefCell;
use std::str::FromStr;

use bitcoin::hashes::Hash;
use bitcoin::{Address, Network, Transaction, Txid};
use bitcoincore_rpc::RpcApi;
use futures::TryFutureExt;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use inscriber::interface::bitcoin_api;
use ord_rs::{Brc20, OrdParser};
use serde::{Deserialize, Serialize};

use crate::constant::{
    BRC20_TICKER_LEN, HTTP_OUTCALL_MAX_RESPONSE_BYTES, HTTP_OUTCALL_PER_CALL_COST,
    HTTP_OUTCALL_REQ_PER_BYTE_COST, HTTP_OUTCALL_RES_DEFAULT_SIZE, HTTP_OUTCALL_RES_PER_BYTE_COST,
};
use crate::interface::bridge_api::BridgeError;
use crate::interface::get_deposit_address;
use crate::interface::store::Brc20TokenInfo;
use crate::state::State;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Rpc;

impl Rpc {
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
        Self::is_valid_btc_address(&holder, network)?;

        let TokenInfo {
            tx_id,
            address,
            ticker,
            ..
        } = Self::get_brc20_token_by_ticker(&indexer_url, &tick)
            .await?
            .token;

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
        let (ic_btc_network, derivation_path) = {
            let state = state.borrow();
            (state.ic_btc_network(), state.derivation_path(None))
        };

        let bridge_addr = get_deposit_address(ic_btc_network, derivation_path).await;

        let brc20_utxo = Self::find_inscription_utxo(
            ic_btc_network,
            bridge_addr,
            reveal_tx_id.as_bytes().to_vec(),
        )
        .map_err(|e| BridgeError::FindInscriptionUtxo(e.to_string()))
        .await?;

        let txid =
            Txid::from_slice(&brc20_utxo.outpoint.txid).expect("failed to convert Txid from slice");

        Ok(Self::get_brc20_transaction_by_id(state, &txid)
            .map_err(|e| BridgeError::GetTransactionById(e.to_string()))?)
    }

    pub(crate) fn parse_and_validate_inscription(
        reveal_tx: Transaction,
    ) -> Result<Brc20, BridgeError> {
        let inscription = OrdParser::parse::<Brc20>(&reveal_tx)
            .map_err(|e| BridgeError::InscriptionParsing(e.to_string()))?;

        match inscription {
            Some(brc20) => {
                let ticker = Self::get_brc20_data(&brc20).1;
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
        base_indexer_url: &str,
        ticker: &str,
    ) -> Result<Brc20TokenResponse, BridgeError> {
        let url = format!("{base_indexer_url}/ordinals/v1/brc-20/tokens/:{ticker}");

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

        let cycles = Self::get_estimated_http_outcall_cycles(&request_params);

        let resp = http_request(request_params, cycles)
            .await
            .map_err(|err| BridgeError::FetchBrc20TokenDetails(format!("{err:?}")))?
            .0;

        log::info!(
            "Indexer responded with: status: {} body: {}",
            resp.status,
            String::from_utf8_lossy(&resp.body)
        );

        serde_json::from_slice(&resp.body).map_err(|err| {
            log::error!("Failed to retrieve BRC20 token from the indexer: {err:?}");
            BridgeError::FetchBrc20TokenDetails(format!("{err:?}"))
        })
    }

    fn get_brc20_transaction_by_id(
        state: &RefCell<State>,
        txid: &Txid,
    ) -> anyhow::Result<Transaction> {
        Ok(state
            .borrow()
            .bitcoin_rpc_client()?
            .get_raw_transaction(txid, None)?)
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
        let network_str = Self::network_as_str(network);

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

        match utxos
            .iter()
            .find(|utxo| utxo.outpoint.txid == txid)
            .cloned()
        {
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
