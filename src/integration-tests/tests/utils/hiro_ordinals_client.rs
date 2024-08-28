use std::collections::HashMap;
use std::str::FromStr as _;

use bitcoin::{Address, Amount};
use brc20_bridge::brc20_info::Brc20Tick;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

/// Ord service client
pub struct HiroOrdinalsClient {
    client: Client,
    url: String,
}

#[derive(Debug)]
/// Outpoint info
pub struct Outpoint {
    pub address: Address,
    pub value: Amount,
}

impl HiroOrdinalsClient {
    #[cfg(feature = "dfx_tests")]
    pub fn dfx_test_client() -> HiroOrdinalsClient {
        Self {
            client: Client::new(),
            url: "http://localhost:8004".to_string(),
        }
    }

    /// Get rune info by id
    pub async fn get_brc20_balances(
        &self,
        address: &Address,
    ) -> anyhow::Result<HashMap<Brc20Tick, u64>> {
        let url = format!("{}/ordinals/v1/brc-20/balances/{address}", self.url);
        let response = self.client.get(&url).send().await?;
        if response.status() != StatusCode::OK {
            return Err(anyhow::anyhow!(
                "Failed to get brc20 balances: {}",
                response.status()
            ));
        }
        let response: GetBrc20BalancesResponse = response.json().await?;
        Ok(response
            .results
            .into_iter()
            .map(|balance| {
                (
                    Brc20Tick::from_str(&balance.ticker).unwrap(),
                    balance.overall_balance,
                )
            })
            .collect())
    }
}

/// Response for `/ordinals/v1/brc-20/balances/{address}` endpoint.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GetBrc20BalancesResponse {
    pub total: u64,
    pub results: Vec<Brc20BalanceResponse>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Brc20BalanceResponse {
    pub ticker: String,
    pub overall_balance: u64,
}
