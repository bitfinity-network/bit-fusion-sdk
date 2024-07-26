use std::collections::HashMap;
use std::str::FromStr as _;

use bitcoin::{Address, Amount};
use ordinals::RuneId;
use reqwest::{Client, StatusCode};
use rune_bridge::rune_info::{RuneInfo, RuneName};
use serde::Deserialize;

pub struct OrdClient {
    client: Client,
    url: String,
}

impl OrdClient {
    pub fn dfx_test_client() -> OrdClient {
        Self {
            client: Client::new(),
            url: "http://localhost:8000".to_string(),
        }
    }

    pub async fn get_rune_info(&self, id: &RuneId) -> anyhow::Result<RuneInfo> {
        let url = format!("{}/rune/{}", self.url, id);

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        if response.status() == StatusCode::NOT_FOUND {
            anyhow::bail!("rune not found");
        }

        let response = response.json::<RuneResponse>().await?;

        Ok(RuneInfo {
            name: RuneName::from_str(&response.entry.spaced_rune)?,
            decimals: response.entry.divisibility,
            block: response.entry.block,
            tx: id.tx,
        })
    }

    pub async fn get_balances(&self, rune_name: &str) -> anyhow::Result<HashMap<String, u64>> {
        let url = format!("{}/runes/balances", self.url);

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        let mut response = response.json::<BalancesResponse>().await?;
        let balances = response
            .runes
            .remove(rune_name)
            .ok_or(anyhow::anyhow!("rune not found in response: {}", rune_name))?;

        Ok(balances.balance)
    }

    pub async fn get_outpoint(&self, outpoint: &str) -> anyhow::Result<Outpoint> {
        let url = format!("{}/output/{}", self.url, outpoint);

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        let response = response.json::<OutpointResponse>().await?;

        let address = Address::from_str(&response.address)
            .map_err(|_| anyhow::anyhow!("invalid address"))?
            .assume_checked();
        let value = Amount::from_sat(response.value);

        Ok(Outpoint { address, value })
    }
}

#[derive(Debug, Deserialize)]
struct RuneResponse {
    entry: Entry,
}

#[derive(Debug, Deserialize)]
struct Entry {
    block: u64,
    divisibility: u8,
    spaced_rune: String,
}

#[derive(Debug, Deserialize)]
struct BalancesResponse {
    #[serde(flatten)]
    runes: HashMap<String, Balances>,
}

#[derive(Debug, Deserialize)]
struct Balances {
    #[serde(flatten)]
    balance: HashMap<String, u64>,
}

#[derive(Debug, Deserialize)]
struct OutpointResponse {
    pub address: String,
    pub value: u64,
}

pub struct Outpoint {
    pub address: Address,
    pub value: Amount,
}
