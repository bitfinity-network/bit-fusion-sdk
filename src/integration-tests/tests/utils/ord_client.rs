use std::str::FromStr as _;

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

        let response = self.client.get(&url).send().await?;

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
