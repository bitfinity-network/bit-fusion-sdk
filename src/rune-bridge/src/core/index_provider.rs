use std::collections::HashMap;
use std::str::FromStr;

use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use ordinals::{RuneId, SpacedRune};
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::interface::{DepositError, OutputResponse};
use crate::rune_info::RuneName;

pub(crate) trait RuneIndexProvider {
    async fn get_rune_amounts(&self, utxo: &Utxo) -> Result<HashMap<RuneName, u128>, DepositError>;
    async fn get_rune_list(&self) -> Result<Vec<(RuneId, SpacedRune, u8)>, DepositError>;
}

const CYCLES_PER_HTTP_REQUEST: u128 = 500_000_000;
const MAX_RESPONSE_BYTES: u64 = 10_000;

pub struct OrdIndexProvider {
    indexer_url: String,
}

impl OrdIndexProvider {
    pub fn new(indexer_url: String) -> Self {
        Self { indexer_url }
    }

    fn indexer_url(&self) -> &str {
        &self.indexer_url
    }

    async fn http_request<R: DeserializeOwned>(&self, uri: &str) -> Result<R, DepositError> {
        let indexer_url = self.indexer_url();
        let url = format!("{indexer_url}/{uri}");

        log::trace!("Sending indexer request to: {url}");

        let request_params = CanisterHttpRequestArgument {
            url,
            max_response_bytes: Some(MAX_RESPONSE_BYTES),
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
            .map_err(|err| DepositError::Unavailable(format!("Indexer unavailable: {err:?}")))?
            .0;

        log::trace!(
            "Indexer responded with: {} {:?} BODY: {}",
            result.status,
            result.headers,
            String::from_utf8_lossy(&result.body)
        );

        serde_json::from_slice(&result.body).map_err(|err| {
            log::error!("Failed to get rune balance from the indexer: {err:?}");
            DepositError::Unavailable(format!("Unexpected response from indexer: {err:?}"))
        })
    }

    pub async fn get_tx_outputs(&self, utxo: &Utxo) -> Result<OutputResponse, DepositError> {
        let outpoint = format_outpoint(&utxo.outpoint);
        log::trace!("get tx output: {}/output/{outpoint}", self.indexer_url());
        self.http_request(&format!("output/{outpoint}")).await
    }
}

impl RuneIndexProvider for OrdIndexProvider {
    async fn get_rune_amounts(&self, utxo: &Utxo) -> Result<HashMap<RuneName, u128>, DepositError> {
        log::trace!(
            "Requesting rune balances for utxo {}:",
            format_outpoint(&utxo.outpoint)
        );
        let response = self.get_tx_outputs(utxo).await?;
        let amounts = response
            .runes
            .iter()
            .filter_map(
                |(spaced_rune, pile)| match RuneName::from_str(&spaced_rune) {
                    Ok(rune_name) => Some((rune_name, pile.amount)),
                    Err(err) => {
                        log::warn!("Failed to parse rune name from the indexer response: {err:?}");
                        None
                    }
                },
            )
            .collect();

        log::trace!(
            "Received rune balances for utxo {}: {:?}",
            format_outpoint(&utxo.outpoint),
            amounts
        );

        Ok(amounts)
    }

    async fn get_rune_list(&self) -> Result<Vec<(RuneId, SpacedRune, u8)>, DepositError> {
        #[derive(Debug, Clone, Deserialize)]
        struct RuneInfo {
            spaced_rune: SpacedRune,
            divisibility: u8,
        }

        #[derive(Debug, Clone, Deserialize)]
        struct RunesResponse {
            entries: Vec<(RuneId, RuneInfo)>,
        }

        // todo: AFAIK this endpoint will return first 50 entries. Need to figure out how to use
        // pagination with this api.
        // https://infinityswap.atlassian.net/browse/EPROD-854
        let response: RunesResponse = self.http_request("runes").await?;

        Ok(response
            .entries
            .into_iter()
            .map(|(rune_id, info)| (rune_id, info.spaced_rune, info.divisibility))
            .collect())
    }
}

fn format_outpoint(outpoint: &Outpoint) -> String {
    // For some reason IC management canister returns bytes of tx_id in reversed order. It is
    // probably related to the fact that WASM uses little endian, but I'm not sure about that.
    // Nevertheless, to get the correct tx_id string we need to reverse the bytes first.
    format!(
        "{}:{}",
        hex::encode(outpoint.txid.iter().copied().rev().collect::<Vec<u8>>()),
        outpoint.vout
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ic_outpoint_formatting() {
        let outpoint = Outpoint {
            txid: vec![
                98, 63, 184, 185, 7, 50, 158, 17, 243, 185, 211, 103, 188, 117, 181, 151, 60, 123,
                6, 92, 153, 208, 7, 254, 73, 104, 37, 139, 72, 22, 74, 26,
            ],
            vout: 2,
        };

        let expected = "1a4a16488b256849fe07d0995c067b3c97b575bc67d3b9f3119e3207b9b83f62:2";
        assert_eq!(&format_outpoint(&outpoint)[..], expected);
    }
}
