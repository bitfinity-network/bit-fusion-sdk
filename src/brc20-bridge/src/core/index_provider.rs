mod hiro;

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::str::FromStr;

use bitcoin::Address;
use bridge_did::brc20_info::{Brc20Info, Brc20Tick};
use ic_exports::ic_cdk::api::management_canister::http_request::{
    CanisterHttpRequestArgument, HttpHeader, HttpMethod, http_request,
};
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;

use self::hiro::{GetBrc20BalancesResponse, GetBrc20TokensResponse};
use crate::interface::DepositError;

pub(crate) trait Brc20IndexProvider {
    /// Get BRC20 balances for the given address.
    async fn get_brc20_balances(
        &self,
        address: &Address,
    ) -> Result<HashMap<Brc20Tick, Decimal>, DepositError>;

    /// Get list of BRC20 tokens.
    async fn get_brc20_tokens(&self) -> Result<HashMap<Brc20Tick, Brc20Info>, DepositError>;
}

const CYCLES_PER_HTTP_REQUEST: u128 = 500_000_000;
const MAX_RESPONSE_BYTES: u64 = 10_000;
const HIRO_MAX_LIMIT: u64 = 60;

/// Trait for a generic HTTP client that can be used to make requests to the indexer.
pub trait HttpClient {
    fn http_request<R: DeserializeOwned>(
        &self,
        url: &str,
        uri: &str,
    ) -> impl Future<Output = Result<R, DepositError>>;
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

/// Implementation of the `RuneIndexProvider` trait that uses the `HttpClient` to make requests to
pub struct OrdIndexProvider<C: HttpClient> {
    client: C,
    indexer_urls: HashSet<String>,
    indexer_consensus_threshold: u8,
}

impl<C> OrdIndexProvider<C>
where
    C: HttpClient,
{
    pub fn new(client: C, indexer_urls: HashSet<String>, indexer_consensus_threshold: u8) -> Self {
        Self {
            client,
            indexer_urls,
            indexer_consensus_threshold,
        }
    }

    /// Get consensus response from the indexer.
    ///
    /// All indexers must return the same response for the same input, other
    /// the function will return an error.
    async fn get_consensus_response<T>(&self, uri: &str) -> Result<T, DepositError>
    where
        T: Clone + DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let mut failed_urls = Vec::with_capacity(self.indexer_urls.len());
        let mut responses: Vec<(String, T)> = Vec::new();
        let mut indexers_agree = true;

        for url in &self.indexer_urls {
            match self.client.http_request::<T>(url, uri).await {
                Ok(response) => {
                    if !responses.is_empty() && responses[0].1 != response {
                        indexers_agree = false;
                    }

                    responses.push((url.clone(), response));
                }
                Err(e) => {
                    log::warn!("Failed to get response from indexer {}: {:?}", url, e);
                    failed_urls.push(url.clone());
                }
            }
        }

        if responses.len() < self.indexer_consensus_threshold as usize {
            Err(DepositError::InsufficientConsensus {
                received_responses: responses.len(),
                required_responses: self.indexer_consensus_threshold,
                checked_indexers: self.indexer_urls.len(),
            })
        } else if !indexers_agree {
            // TODO: After https://infinityswap.atlassian.net/browse/EPROD-971 is done, return
            // actual values here instead of formated response
            Err(DepositError::IndexersDisagree {
                indexer_responses: responses
                    .into_iter()
                    .map(|(url, response)| (url, format!("{response:?}")))
                    .collect(),
            })
        } else {
            Ok(responses.pop().expect("responses vector is empty").1)
        }
    }
}

impl<C> Brc20IndexProvider for OrdIndexProvider<C>
where
    C: HttpClient,
{
    async fn get_brc20_balances(
        &self,
        address: &Address,
    ) -> Result<HashMap<Brc20Tick, Decimal>, DepositError> {
        let mut balances = HashMap::new();
        let mut offset = 0;
        let mut total = usize::MAX;

        while offset < total {
            let uri = format!(
                "/ordinals/v1/brc-20/balances/{address}?offset={offset}&limit={HIRO_MAX_LIMIT}"
            );
            let response = self
                .get_consensus_response::<GetBrc20BalancesResponse>(&uri)
                .await?;

            // update total
            total = response.total as usize;
            // increment offset
            offset += response.results.len();

            for result in response.results {
                let tick = Brc20Tick::from_str(&result.ticker).map_err(|_| {
                    DepositError::Unavailable(format!(
                        "Invalid BRC20 token ticker: {}",
                        result.ticker
                    ))
                })?;

                balances.insert(tick, result.overall_balance);
            }
        }

        Ok(balances)
    }

    async fn get_brc20_tokens(&self) -> Result<HashMap<Brc20Tick, Brc20Info>, DepositError> {
        let mut tokens = HashMap::new();
        let mut offset = 0;
        let mut total = usize::MAX;

        while offset < total {
            let uri = format!("/ordinals/v1/brc-20/tokens?offset={offset}&limit={HIRO_MAX_LIMIT}");
            let response = self
                .get_consensus_response::<GetBrc20TokensResponse>(&uri)
                .await?;

            // update total
            total = response.total as usize;
            // increment offset
            offset += response.results.len();

            for result in response.results {
                let tick = Brc20Tick::from_str(&result.ticker).map_err(|_| {
                    DepositError::Unavailable(format!(
                        "Invalid BRC20 token ticker: {}",
                        result.ticker
                    ))
                })?;

                tokens.insert(
                    tick,
                    Brc20Info {
                        tick,
                        decimals: result.decimals,
                    },
                );
            }
        }

        Ok(tokens)
    }
}
