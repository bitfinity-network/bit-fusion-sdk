use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::str::FromStr as _;

use async_trait::async_trait;
use bridge_did::init::IndexerType;
use bridge_did::runes::RuneName;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{Outpoint, Utxo};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    CanisterHttpRequestArgument, HttpHeader, HttpMethod, http_request,
};
use ordinals::{RuneId, SpacedRune};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::core::rune_inputs::GetInputsError;
use crate::interface::{DepositError, OutputResponse};

#[async_trait(?Send)]
pub(crate) trait RuneIndexProvider {
    /// Get amounts of all runes in the given UTXO.
    async fn get_rune_amounts(
        &self,
        utxo: &Utxo,
    ) -> Result<HashMap<RuneName, u128>, GetInputsError>;
    /// Get the list of all runes in the indexer
    async fn get_rune_list(&self) -> Result<Vec<(RuneId, SpacedRune, u8)>, GetInputsError>;
}

pub(crate) fn get_indexer(indexer_type: IndexerType) -> Box<dyn RuneIndexProvider> {
    match indexer_type {
        IndexerType::OrdHttp { url } => Box::new(OrdIndexProvider::new(IcHttpClient, url)),
    }
}

const CYCLES_PER_HTTP_REQUEST: u128 = 500_000_000;
const MAX_RESPONSE_BYTES: u64 = 10_000;

/// Trait for a generic HTTP client that can be used to make requests to the indexer.
pub trait HttpClient {
    fn http_request<R: DeserializeOwned>(
        &self,
        url: &str,
        uri: &str,
    ) -> impl Future<Output = Result<R, DepositError>> + Send;
}

/// HTTP client implementation for the Internet Computer canisters.
pub struct IcHttpClient;

impl HttpClient for IcHttpClient {
    async fn http_request<R: DeserializeOwned>(
        &self,
        url: &str,
        uri: &str,
    ) -> Result<R, DepositError> {
        let url = format!("{url}/{}", uri.trim_start_matches('/'));

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RuneInfo {
    spaced_rune: SpacedRune,
    divisibility: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RunesResponse {
    entries: Vec<(RuneId, RuneInfo)>,
    next: Option<u64>,
}

/// Implementation of the `RuneIndexProvider` trait that uses the `HttpClient` to make requests to
pub struct OrdIndexProvider<C: HttpClient> {
    client: C,
    url: String,
}

impl<C> OrdIndexProvider<C>
where
    C: HttpClient,
{
    pub fn new(client: C, url: String) -> Self {
        Self { client, url }
    }
}

#[async_trait(?Send)]
impl<C> RuneIndexProvider for OrdIndexProvider<C>
where
    C: HttpClient,
{
    async fn get_rune_amounts(
        &self,
        utxo: &Utxo,
    ) -> Result<HashMap<RuneName, u128>, GetInputsError> {
        let outpoint = format_outpoint(&utxo.outpoint);
        log::trace!("Requesting rune balances for utxo: {outpoint}",);

        let uri = format!("output/{outpoint}");
        let response: OutputResponse = self
            .client
            .http_request(&self.url, &uri)
            .await
            .map_err(|err| GetInputsError::IndexerError(format!("{err:?}")))?;

        let amounts = response
            .runes
            .iter()
            .filter_map(
                |(spaced_rune, pile)| match RuneName::from_str(spaced_rune) {
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

    async fn get_rune_list(&self) -> Result<Vec<(RuneId, SpacedRune, u8)>, GetInputsError> {
        let mut page = 0;
        let mut entries = vec![];

        loop {
            let uri = format!("runes/{page}");
            let response: RunesResponse = self
                .client
                .http_request(&self.url, &uri)
                .await
                .map_err(|err| GetInputsError::IndexerError(format!("{err:?}")))?;
            entries.extend(response.entries);

            if let Some(next) = response.next {
                page = next;
            } else {
                break;
            }
        }

        Ok(entries
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

    use ordinals::Rune;

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

    #[tokio::test]
    async fn test_should_get_all_runes() {
        let mut runes = HashMap::new();
        runes.insert(
            0u64,
            vec![
                (
                    RuneId { block: 1, tx: 1 },
                    RuneInfo {
                        spaced_rune: SpacedRune {
                            rune: Rune(0u128),
                            spacers: 1,
                        },
                        divisibility: 8,
                    },
                ),
                (
                    RuneId { block: 1, tx: 2 },
                    RuneInfo {
                        spaced_rune: SpacedRune {
                            rune: Rune(1u128),
                            spacers: 1,
                        },
                        divisibility: 8,
                    },
                ),
            ],
        );
        runes.insert(
            1u64,
            vec![(
                RuneId { block: 2, tx: 1 },
                RuneInfo {
                    spaced_rune: SpacedRune {
                        rune: Rune(2u128),
                        spacers: 1,
                    },
                    divisibility: 8,
                },
            )],
        );

        let client = MockHttpClient { runes };
        let provider = OrdIndexProvider::new(client, "http://localhost:8080".into());

        let runes = provider.get_rune_list().await.unwrap();
        assert_eq!(runes.len(), 3);
        assert_eq!(runes[0].0, RuneId { block: 1, tx: 1 });
        assert_eq!(runes[1].0, RuneId { block: 1, tx: 2 });
        assert_eq!(runes[2].0, RuneId { block: 2, tx: 1 });
    }

    struct MockHttpClient {
        /// Runes by page
        runes: HashMap<u64, Vec<(RuneId, RuneInfo)>>,
    }

    impl HttpClient for MockHttpClient {
        async fn http_request<R: DeserializeOwned>(
            &self,
            _url: &str,
            uri: &str,
        ) -> Result<R, DepositError> {
            let page = uri
                .strip_prefix("runes/")
                .and_then(|page| page.parse::<u64>().ok())
                .expect("Invalid URI");

            let response = RunesResponse {
                entries: self.runes.get(&page).cloned().unwrap_or_default(),
                next: self.runes.contains_key(&(page + 1)).then(|| page + 1),
            };

            let serialized =
                serde_json::to_string(&response).expect("Failed to serialize response");

            Ok(serde_json::from_str(&serialized).expect("Failed to deserialize response"))
        }
    }
}
