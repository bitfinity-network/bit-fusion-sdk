use std::collections::HashMap;
use std::str::FromStr as _;

use bitcoin::{Address, Amount};
use bridge_did::brc20_info::{Brc20Info, Brc20Tick};
use reqwest::{Client, StatusCode};
use rust_decimal::Decimal;
use serde::Deserialize;

use super::token_amount::TokenAmount;

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
    pub fn dfx_test_client() -> HiroOrdinalsClient {
        Self {
            client: Client::new(),
            url: "http://localhost:8004".to_string(),
        }
    }

    async fn get_brc20_tokens(&self) -> anyhow::Result<HashMap<Brc20Tick, Brc20Info>> {
        let mut tokens = HashMap::new();
        let mut offset = 0;
        let mut total = usize::MAX;

        while offset < total {
            let url: String = format!(
                "{}/ordinals/v1/brc-20/tokens?offset={offset}&limit=60",
                self.url
            );
            let response = self.client.get(&url).send().await?;
            if response.status() != StatusCode::OK {
                return Err(anyhow::anyhow!(
                    "Failed to get brc20 balances: {}",
                    response.status()
                ));
            }
            let response: GetBrc20TokensResponse = response.json().await?;

            // update total
            total = response.total as usize;
            // increment offset
            offset += response.results.len();

            for result in response.results {
                let tick = Brc20Tick::from_str(&result.ticker).map_err(|_| {
                    anyhow::anyhow!("Invalid BRC20 token ticker: {}", result.ticker)
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

    /// Get rune info by id
    pub async fn get_brc20_balances(
        &self,
        address: &Address,
    ) -> anyhow::Result<HashMap<Brc20Tick, TokenAmount>> {
        let token_infos = self.get_brc20_tokens().await?;

        let url: String = format!("{}/ordinals/v1/brc-20/balances/{address}", self.url);
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
                let integer_balance = Self::integer_amount(
                    balance.overall_balance,
                    token_infos
                        .get(&Brc20Tick::from_str(&balance.ticker).unwrap())
                        .unwrap()
                        .decimals,
                );

                (
                    Brc20Tick::from_str(&balance.ticker).unwrap(),
                    integer_balance,
                )
            })
            .collect())
    }

    fn integer_amount(amount: Decimal, decimals: u8) -> TokenAmount {
        use rust_decimal::prelude::ToPrimitive;
        let multiplier = 10u64.pow(decimals as u32);
        let decimals_amount = (amount * Decimal::from(multiplier))
            .trunc()
            .to_u128()
            .expect("Failed to convert to u64");

        TokenAmount::from_decimals(decimals_amount, decimals)
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
    pub overall_balance: Decimal,
}

/// Response for `/ordinals/v1/brc-20/tokens` endpoint.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GetBrc20TokensResponse {
    pub total: u64,
    pub results: Vec<Brc20TokenResponse>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Brc20TokenResponse {
    pub ticker: String,
    pub decimals: u8,
}
