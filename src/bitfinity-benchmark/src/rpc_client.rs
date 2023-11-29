use std::time::Duration;

use anyhow::Context;
use ethereum_types::{H160, H256, U256};
use ethers_core::types::{Block, BlockNumber, Bytes, Transaction, TransactionReceipt};
use serde_json::{json, Value};
use thiserror::Error;

const AMOUNT_TO_MINT: u128 = 10_u128.pow(18);

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("Bad HTTP status: {0}")]
    BadStatus(reqwest::StatusCode),
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Unexpected value: {0}")]
    UnexpectedValue(String),
}

/// Mint tokens to the given address
pub async fn mint_tokens(url: &str, address: H160) -> anyhow::Result<()> {
    // Amount to mint in Hex
    let amount = U256::from(AMOUNT_TO_MINT);
    let request = json!({"jsonrpc":"2.0","method":"ic_mintNativeToken","params":[
        address, amount
    ], "id":1});

    reqwest::Client::new()
        .post(url)
        .json(&request)
        .send()
        .await
        .context("Failed to mint tokens")?
        .json::<Value>()
        .await
        .context("Failed to mint tokens")?;

    log::info!("Minted {} tokens to {}", AMOUNT_TO_MINT, address);

    Ok(())
}

/// Send a batch of transactions to the given url
pub async fn send_transactions_batch(
    url: &str,
    transactions: impl Iterator<Item = &Bytes>,
) -> anyhow::Result<Vec<H256>> {
    let data = Value::Array(
        transactions
            .enumerate()
            .map(|(index, transaction)| {
                json!({"jsonrpc":"2.0","id":index,"method":"eth_sendRawTransaction","params":[
                    transaction,
                ]})
            })
            .collect(),
    );

    let response = reqwest::Client::new()
        .post(url)
        .timeout(Duration::from_secs(10))
        .json(&data)
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Bad response status: {}", response.status());
    }

    let response = response
        .json::<Value>()
        .await
        .context("Failed to decode eth_sendRawTransaction response")?;

    let response = response
        .as_array()
        .expect("eth_sendRawTransaction batch response should be an array");

    let mut tx_hashes = Vec::with_capacity(response.len());

    for entry in response {
        let entry = &entry["result"];
        if !entry.is_null() {
            tx_hashes
                .push(serde_json::from_value::<H256>(entry.clone()).context("bad entry value")?);
        }
    }

    Ok(tx_hashes)
}

/// Send a transaction to the given url
pub async fn send_transaction(url: &str, transaction: &Bytes) -> anyhow::Result<H256> {
    let request = json!({"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":[
        transaction,
    ],"id":1});

    let response = reqwest::Client::new()
        .post(url)
        .timeout(Duration::from_secs(10))
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Bad response status: {}", response.status());
    }

    let response = response
        .json::<Value>()
        .await
        .context("Failed to decode eth_sendRawTransaction response")?;

    let tx_hash = response["result"].clone();

    let tx_hash = serde_json::from_value::<H256>(tx_hash)
        .context("Failed to deserialize transaction hash")?;

    Ok(tx_hash)
}

/// Get transaction receipt given a transaction hash
pub async fn get_transaction_receipt(
    url: &str,
    tx_hash: &H256,
) -> anyhow::Result<Option<TransactionReceipt>> {
    let request = json!({"jsonrpc":"2.0","method":"eth_getTransactionReceipt","params":[
        tx_hash
    ],"id":1});

    let response = reqwest::Client::new()
        .post(url)
        .json(&request)
        .send()
        .await
        .context("Failed to get transaction receipt")?
        .json::<Value>()
        .await
        .context("Failed to get transaction receipt")?;

    let receipt = response["result"].clone();

    let receipt = if receipt.is_null() {
        None
    } else {
        Some(
            serde_json::from_value::<TransactionReceipt>(receipt)
                .context("Failed to deserialize transaction receipt")?,
        )
    };

    Ok(receipt)
}

/// Get Block by number
pub async fn get_block_by_number(
    url: &str,
    block_number: BlockNumber,
) -> anyhow::Result<Option<Block<Transaction>>> {
    let request = json!({"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":[
        block_number,
        true
    ],"id":1});

    let response = reqwest::Client::new()
        .post(url)
        .json(&request)
        .send()
        .await
        .context("Failed to get transaction receipt")?
        .json::<Value>()
        .await
        .context("Failed to get transaction receipt")?;

    let block = response["result"].clone();

    let maybe_block = if block.is_null() {
        None
    } else {
        Some(
            serde_json::from_value::<Block<Transaction>>(block)
                .context("Failed to deserialize block")?,
        )
    };

    Ok(maybe_block)
}
