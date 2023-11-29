use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use bitfinity_benchmark::{rpc_client, stats};
use clap::Parser;
use csv::Writer;
use env_logger::Builder;
use ethers_core::types::{Bytes, TransactionReceipt};
use futures::future;
use log::{LevelFilter, SetLoggerError};
use tokio::sync::mpsc;
use tokio::time;

/// Simple CLI program for Benchmarking BitFinity Network
#[derive(Parser, Debug)]
#[clap(version = "0.1", about = "Tool for benchmarking BitFinity Network")]
struct BenchmarkArgs {
    /// Test time in seconds
    #[arg(long, short('t'))]
    test_time: u64,

    /// The rate of transactions to be sent per second. Default is 10.
    #[arg(long, short('r'))]
    rate: Option<u64>,

    /// Fetch receipts for transactions - This will measure the time it takes
    /// for a transaction to be included in a block. Default is false.
    #[arg(long)]
    fetch_receipts: Option<bool>,

    /// The RPC URL to send transactions to
    #[arg(long, short('u'))]
    url: String,

    /// The output folder to write the results to
    #[arg(long, short('o'))]
    output: Option<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    init_logger()?;

    let args = BenchmarkArgs::parse();

    let test_time = args.test_time;
    let rate = args.rate.unwrap_or(10);
    let rpc_url = args.url;
    let output = args.output.unwrap_or("./target".to_string());
    let fetch_receipts = args.fetch_receipts.unwrap_or(false);

    log::info!("BitFinity Benchmarking");
    log::info!("----------------------");
    log::info!("- test-time: {}", test_time);
    log::info!("- rate: {:?}", rate);
    log::info!("- rpc-url: {:?}", rpc_url);
    log::info!("- output: {:?}", output);
    log::info!("- fetch-receipts: {:?}", fetch_receipts);
    log::info!("----------------------");

    let total_transactions = rate * test_time;
    log::info!("Total Transactions: {}", total_transactions);

    log::info!("Starting pre-benchmark phase");
    let txns = pre_benchmark_preparation(total_transactions, &rpc_url).await?;
    log::info!("Pre-benchmark phase complete");

    log::info!("Starting benchmarking");

    stats(&rpc_url, rate, test_time, &output, txns, fetch_receipts).await?;

    log::info!("Benchmarking complete");

    Ok(())
}

/// Initializes the logger
fn init_logger() -> Result<(), SetLoggerError> {
    // Initialize logger
    Builder::new()
        .filter_level(LevelFilter::Info)
        .parse_filters("info, bitfinity-benchmark=info")
        .try_init()?;
    Ok(())
}

/// Pre-benchmark preparation
async fn pre_benchmark_preparation(txns: u64, url: &str) -> anyhow::Result<Vec<Bytes>> {
    // create transactions
    let mut transactions = Vec::new();

    for _ in 0..txns {
        let url_ref = url.to_string();
        let transaction = tokio::spawn(async move {
            stats::random_transaction(&url_ref)
                .await
                .expect("Failed to create transaction")
        });
        transactions.push(transaction);

        // Add a delay to avoid overloading the node
        time::sleep(Duration::from_millis(50)).await;
    }

    let txns = future::join_all(transactions)
        .await
        .into_iter()
        .map(|transaction| transaction.expect("Failed to create transaction"))
        .collect::<Vec<Bytes>>();

    Ok(txns)
}

pub async fn stats(
    rpc_url: &str,
    rate: u64,
    test_time: u64,
    output: &str,
    transactions: Vec<Bytes>,
    fetch_receipts: bool,
) -> anyhow::Result<()> {
    log::info!("Sending transactions at {} txns/sec", rate);

    let file_path = stats::create_output_file(rate, test_time, output, "BenchmarkStats")?;

    let mut writer = Writer::from_path(file_path).context("Failed to create CSV file")?;

    let counter = Arc::new(AtomicUsize::new(0));

    let (tx, mut rx) = mpsc::unbounded_channel::<(Option<TransactionReceipt>, usize, u64, u64)>();

    let mut tasks = Vec::new();

    let mut interval = time::interval(Duration::from_secs(1));
    let start = Instant::now();

    let batch_size = 10;

    for transactions_block in transactions.chunks(rate as usize) {
        interval.tick().await;

        for transactions_block in transactions_block.chunks(batch_size) {
            let transactions_block: Vec<(usize, Bytes)> = transactions_block
                .iter()
                .map(|tx| {
                    let counter_ref = Arc::clone(&counter);
                    let i = counter_ref.fetch_add(1, Ordering::Release);
                    (i, tx.clone())
                })
                .collect();

            let rpc_url = rpc_url.to_string();
            let tx = tx.clone();

            tasks.push(tokio::spawn(async move {
                let start = stats::now();

                let hashes = match rpc_client::send_transactions_batch(
                    &rpc_url,
                    transactions_block.iter().map(|(_, t)| t),
                )
                .await
                {
                    Ok(hash) => hash,
                    Err(e) => {
                        log::warn!("Error sending transactions. Err: {e:?}");
                        Err(e)?
                    }
                };

                for (index, hash) in hashes.into_iter().enumerate() {
                    let i = transactions_block
                        .get(index)
                        .map(|(i, _)| *i)
                        .unwrap_or_default();
                    let rpc_url = rpc_url.to_string();
                    let tx = tx.clone();

                    let mut receipt_tasks = Vec::new();

                    receipt_tasks.push(tokio::spawn(async move {
                        let receipt = if fetch_receipts {
                            stats::poll_transaction_receipt(&rpc_url, &hash).await?
                        } else {
                            None
                        };

                        let end = stats::now();
                        tx.send((receipt, i, start, end))?;
                        anyhow::Ok(())
                    }));
                    let _ = future::join_all(receipt_tasks).await;
                }

                anyhow::Ok(())
            }));
        }
    }

    let result = future::join_all(tasks).await;
    let tx_send_errors = result
        .into_iter()
        .filter(|f| f.is_err() || f.as_ref().unwrap().is_err())
        .count();
    let end_time = start.elapsed().as_secs_f64();

    drop(tx);

    let total_transactions = counter.load(Ordering::Acquire);
    let mut success_tx = 0;
    let mut failed_tx = 0;

    while let Some((receipt, i, start, end)) = rx.recv().await {
        if fetch_receipts {
            match receipt {
                Some(_) => success_tx += 1,
                None => failed_tx += 1,
            }
        }

        writer
            .write_record(&[(i + 1).to_string(), start.to_string(), end.to_string()])
            .context("Failed to write to CSV file")?;
    }
    writer.flush()?;

    log::info!("----------------- Statistics ---------------");
    log::info!("Total Transactions Sent: {}", total_transactions);
    log::info!("Total Transaction Send Failures: {}", tx_send_errors);
    if fetch_receipts {
        log::info!("Total Successful Transactions: {}", success_tx);
        log::info!("Total Transaction Process Failures: {}", failed_tx);
        log::info!(
            "Effectiveness: {:.1}% (success_tx/total_transactions)",
            (success_tx as f64 / total_transactions as f64) * 100.0
        );
    }
    log::info!("Time(s) : {:.1}", end_time);
    log::info!(
        "Successful TXs Rate {:.1} tx/s",
        success_tx as f64 / end_time
    );
    log::info!(
        "Total TXs Rate      {:.1} tx/s",
        total_transactions as f64 / end_time
    );
    log::info!("------------------------------------------------------");

    Ok(())
}
