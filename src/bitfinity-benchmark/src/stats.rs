use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Ok};
use eth_signer::{LocalWallet, Signer};
use ethereum_types::{H160, H256};
use ethers_core::types::transaction::eip2718::TypedTransaction;
use ethers_core::types::{Bytes, TransactionReceipt, TransactionRequest};

use crate::rpc_client;

/// The timeout for retrieving a transaction receipt
const TIMEOUT: Duration = Duration::from_secs(2);

/// This function returns the current time in milliseconds
pub fn now() -> u64 {
    let now = SystemTime::now();
    let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    duration_since_epoch.as_millis() as u64
}

/// Returns a new random transactions which we mint for the sender
pub async fn random_transaction(url: &str) -> anyhow::Result<Bytes> {
    let private_key = H256::random();
    let wallet =
        LocalWallet::from_bytes(private_key.as_bytes()).context("Failed to create wallet")?;
    let from = wallet.address();

    let to = H160::zero();
    let tx: TypedTransaction = TransactionRequest::new()
        .from(from)
        .to(to)
        .value(0)
        .chain_id(355113)
        .nonce(0)
        .gas_price(500000000)
        .gas(53000)
        .into();

    let signature = wallet
        .sign_transaction(&tx)
        .await
        .context("Failed to sign transaction.")?;

    let bytes = tx.rlp_signed(&signature);
    let url_ref = url.to_string();

    rpc_client::mint_tokens(&url_ref, from)
        .await
        .context("Failed to mint tokens")?;

    Ok(bytes)
}

/// Generate file name
pub fn create_output_file(
    rate: u64,
    test_time: u64,
    output: &str,
    file_type: &str,
) -> anyhow::Result<String> {
    let name = format!(
        "{}-txnsPerSec-{}-seconds-{}.csv",
        rate, test_time, file_type
    );

    let path = Path::new(&output);
    let file_path = path.join(name);
    File::create(&file_path).context("Failed to create CSV file")?;
    let file_path_name = file_path.display().to_string();
    Ok(file_path_name)
}

/// This function polls the transaction receipt until it is available, or until
/// the timeout is reached
pub async fn poll_transaction_receipt(
    rpc_url: &str,
    tx_hash: &H256,
) -> anyhow::Result<Option<TransactionReceipt>> {
    let start = Instant::now();

    loop {
        let receipt = rpc_client::get_transaction_receipt(rpc_url, tx_hash).await?;

        if receipt.is_some() {
            break Ok(receipt);
        }

        if start.elapsed() > TIMEOUT {
            break Ok(None);
        }
    }
}
