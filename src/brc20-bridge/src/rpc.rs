use std::cell::RefCell;
use std::str::FromStr;

use anyhow::Ok;
use bitcoin::absolute::LockTime;
use bitcoin::transaction::Version;
use bitcoin::{
    Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use serde::Deserialize;

use crate::api::BridgeError;
use crate::constant::{CYCLES_PER_HTTP_REQUEST, MAX_HTTP_RESPONSE_BYTES};
use crate::state::State;

/// Retrieves (and re-constructs) the reveal transaction by its ID.
///
/// We use the reveal transaction (as opposed to the commit transaction)
/// because it contains the actual BRC20 inscription that needs to be parsed.
pub async fn fetch_reveal_transaction(
    state: &RefCell<State>,
    reveal_tx_id: &str,
) -> anyhow::Result<Transaction> {
    let (network, indexer_url) = {
        let state = state.borrow();
        (state.btc_network(), state.indexer_url())
    };

    let network_str = match network {
        Network::Testnet => "/testnet",
        Network::Regtest => "/regtest",
        Network::Signet => "/signet",
        _ => "",
    };

    let url = format!("{indexer_url}{network_str}/api/tx/{reveal_tx_id}");

    log::trace!("Retrieving reveal transaction from: {url}");

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

    log::trace!(
        "Response from indexer: Status: {} Headers: {:?} Body: {}",
        result.status,
        result.headers,
        String::from_utf8_lossy(&result.body)
    );

    let tx: TxInfo = serde_json::from_slice(&result.body).map_err(|err| {
        log::error!("Failed to retrieve the reveal transaction from the indexer: {err:?}");
        BridgeError::GetTransactionById(format!("{err:?}"))
    })?;

    tx.try_into()
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
