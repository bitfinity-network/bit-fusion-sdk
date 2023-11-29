use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::Context;
use bitfinity_benchmark::{ic_client, rpc_client, stats};
use candid::Principal;
use clap::Parser;
use csv::Writer;
use env_logger::Builder;
use ethereum_types::{H256, U64};
use ethers_core::types::{Block, BlockNumber, Transaction, TransactionReceipt};
use evm_canister_client::IcAgentClient;
use log::LevelFilter;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::time;

const MAX_TX_PER_BLOCK: usize = 25;

/// Simple CLI program for Benchmarking BitFinity Network
#[derive(Parser, Debug)]
#[clap(version = "0.1", about = "Tool for benchmarking BitFinity Network")]
struct BenchmarkArgs {
    /// Identity for Canister client. If not provided the canister metrics won't be collected
    #[arg(long, short('i'))]
    identity: Option<PathBuf>,

    /// Fetch receipts for transactions - This will measure the time it takes
    /// for a transaction to be included in a block. Default is false.
    #[arg(long, short('f'), default_value = "false")]
    fetch_receipts: bool,

    /// The RPC URL to send transactions to
    #[arg(long, short('u'))]
    url: String,

    /// Evmc canister principal
    #[arg(long = "evmc")]
    evmc: Principal,

    /// Ethereum main net RPC URL
    #[arg(long, default_value = "https://cloudflare-eth.com/")]
    ethereum_url: String,

    /// The output file to write the results to
    #[arg(long, short('o'))]
    output: Option<PathBuf>,

    #[arg(long, short('n'), default_value = "ic")]
    network: String,

    /// block to start with ("0xb443" is the first block with a transaction in ethereum)
    #[arg(long, short('s'), default_value = "0xb443")]
    start_block: String,

    /// transaction processing interval (ms)
    #[arg(long, short('I'), default_value = "1000")]
    transaction_processing_interval: u64,
}

/// Transaction execution result
struct TxResult {
    start: u64,
    end: u64,
    receipt: Option<TransactionReceipt>,
}

impl TxResult {
    pub fn new(start: u64) -> Self {
        Self {
            start,
            end: stats::now(),
            receipt: None,
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    init_logger()?;
    let args = BenchmarkArgs::parse();

    let rpc_url = args.url;
    let network = args.network;
    let ethereum_url = args.ethereum_url;
    let output_file = args
        .output
        .unwrap_or(PathBuf::from("./target/replicate-ethereum-mainnet.csv"));
    let fetch_receipts = args.fetch_receipts;
    let start_block = u64::from_str_radix(&args.start_block.replace("0x", ""), 16)?;
    let transaction_processing_interval =
        Duration::from_millis(args.transaction_processing_interval);

    log::info!("BitFinity Benchmarking");
    log::info!("----------------------");
    log::info!("- rpc-url: {rpc_url}");
    log::info!("- rpc-url: {ethereum_url}");
    log::info!("- output: {}", output_file.display());
    log::info!("- network: {network}");
    log::info!("- fetch-receipts: {fetch_receipts}");
    log::info!("- start-block: {start_block:#x}");
    log::info!("----------------------");

    log::info!("Initializing IC agent");
    let client = match args.identity {
        Some(identity_path) => Some(
            IcAgentClient::with_identity(args.evmc, &identity_path, &network, None)
                .await
                .expect("failed to init IcAgent"),
        ),
        None => None,
    };
    log::info!("IC agent initialized, starting benchmarking");

    stats(
        ethereum_url,
        rpc_url,
        &output_file,
        client,
        fetch_receipts,
        start_block,
        transaction_processing_interval,
    )
    .await?;

    log::info!("Benchmarking complete");

    Ok(())
}

/// Initializes the logger
fn init_logger() -> anyhow::Result<()> {
    // Initialize logger
    Builder::new()
        .filter_level(LevelFilter::Info)
        .parse_filters("info")
        .try_init()?;
    Ok(())
}

async fn stats(
    ethereum_url: String,
    rpc_url: String,
    output: &Path,
    client: Option<IcAgentClient>,
    fetch_receipts: bool,
    start_block: u64,
    transaction_processing_interval: Duration,
) -> anyhow::Result<()> {
    let mut block_number = BlockNumber::Number(start_block.into());

    let start: Instant = Instant::now();
    let mut total_transactions = 0;

    let mut writer = Writer::from_writer(create_output_file(output)?);

    // counters
    let mut success_tx = 0;
    let mut failed_tx = 0;

    let (block_tx, mut block_rx) = mpsc::unbounded_channel::<Block<Transaction>>();

    // start ethereum block worker
    log::info!("starting Ethereum block worker...");
    let block_worker_task = tokio::spawn(async move {
        let ethereum_url = ethereum_url.clone();
        log::info!(
            "Ethereum block worker started crawling blocks from {block_number} at {ethereum_url}"
        );
        let mut interval = time::interval(Duration::from_millis(100));
        loop {
            let block = rpc_client::get_block_by_number(&ethereum_url, block_number).await?;
            if let Some(block) = block {
                // if block has transactions, send block and wait interval
                if !block.transactions.is_empty() {
                    log::debug!(
                        "Found {} transactions in block {block_number}",
                        block.transactions.len()
                    );
                    block_tx.send(block)?;
                    interval.tick().await;
                }
            } else {
                break;
            }

            // increment block
            block_number = block_number
                .as_number()
                .map(|x| x + U64::from(1_u64))
                .map(BlockNumber::from)
                .unwrap();
        }
        anyhow::Ok(())
    });

    let mut should_stop = false;
    // iter over blocks, untile next is None
    while !should_stop {
        let mut blocks = vec![];

        // collect blocks until queue is empty
        loop {
            match block_rx.try_recv() {
                Ok(block) => {
                    log::info!(
                        "executing {} transactions from block {:#x}",
                        block.transactions.len(),
                        block.number.unwrap_or_default(),
                    );
                    total_transactions += block.transactions.len();
                    blocks.push(block);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    should_stop = true;
                    break;
                }
            }
        }

        // send transactions
        let transactions: Vec<Transaction> = blocks
            .into_iter()
            .flat_map(|block| block.transactions.to_vec())
            .collect();

        // if no transaction wait and continue
        if transactions.is_empty() {
            sleep(Duration::from_millis(100));
            continue;
        }

        // execute
        let mut tx_results =
            send_transactions(&transactions, &rpc_url, transaction_processing_interval).await;

        // collect receipts
        if fetch_receipts {
            get_receipts(&rpc_url, &mut tx_results).await
        }
        // fetch metrics
        let metrics = if let Some(client) = client.as_ref() {
            Some(ic_client::get_metrics(client).await?)
        } else {
            None
        };
        // iter over hashes
        for (hash, result) in tx_results {
            match result.receipt {
                Some(_) => success_tx += 1,
                None => failed_tx += 1,
            }

            let status = match (fetch_receipts, result.receipt) {
                (false, _) => "IGN",
                (true, None) => "NOK",
                (true, Some(_)) => "OK",
            };

            let hash = format!("0x{}", hex::encode(hash.as_fixed_bytes()));
            let stable_memory_size = metrics
                .as_ref()
                .map(|metrics| metrics.stable_memory_size)
                .unwrap_or_default();
            let heap_memory_size = metrics
                .as_ref()
                .map(|metrics| metrics.heap_memory_size)
                .unwrap_or_default();

            // Record: hash,{OK,NOK,IGN},start_time,end_time,stable_memory_size,heap_memory_size
            writer
                .write_record([
                    &hash,
                    status,
                    &result.start.to_string(),
                    &result.end.to_string(),
                    &stable_memory_size.to_string(),
                    &heap_memory_size.to_string(),
                ])
                .context("Failed to write to CSV file")?;

            log::info!(
                "block {block_number}: {hash},{status},{},{},{stable_memory_size},{heap_memory_size}", result.start, result.end
            );

            writer.flush()?;
        }
    }

    let _ = tokio::join!(block_worker_task);

    let end_time = start.elapsed().as_secs_f64();

    log::info!("----------------- Statistics ---------------");
    log::info!("Total Transactions Sent: {}", total_transactions);
    if fetch_receipts {
        log::info!("Total Successful Transactions: {}", success_tx);
        log::info!("Total Failed Transactions: {}", failed_tx);
        log::info!(
            "Effectiveness: {:.1}%",
            (success_tx as f64 / total_transactions as f64) * 100.0
        );
    }
    log::info!("Time(s) : {:.1}", end_time);
    log::info!("Rate {:.1} tx/s", total_transactions as f64 / end_time);
    log::info!("------------------------------------------------------");

    Ok(())
}

async fn send_transactions(
    transactions: &[Transaction],
    rpc_url: &str,
    transaction_processing_interval: Duration,
) -> HashMap<H256, TxResult> {
    log::info!("sending {} transactions...", transactions.len());
    let mut tx_results = HashMap::with_capacity(transactions.len());
    for (index, tx) in transactions.iter().enumerate() {
        let start = stats::now();
        let hash = format!("0x{}", hex::encode(tx.hash.as_fixed_bytes()));
        let from = format!("0x{}", hex::encode(tx.from.as_fixed_bytes()));
        let to = format!(
            "0x{}",
            tx.to
                .map(|to| hex::encode(to.as_fixed_bytes()))
                .unwrap_or_default()
        );
        log::info!("sending transaction {hash} from {from} to {to}");
        match rpc_client::send_transaction(rpc_url, &tx.rlp()).await {
            Ok(hash) => {
                tx_results.insert(hash, TxResult::new(start));
            }
            Err(err) => {
                log::info!("Transaction {hash} failed: {err}");
                tx_results.insert(tx.hash, TxResult::new(start));
            }
        }
        // if we've reached max tx per block or is the last one, sleep to give EVMC the time to process
        if index % MAX_TX_PER_BLOCK == 0 || index == transactions.len() - 1 {
            sleep(transaction_processing_interval);
        }
    }

    tx_results
}

async fn get_receipts(rpc_url: &str, tx_results: &mut HashMap<H256, TxResult>) {
    for (hash, result) in tx_results.iter_mut() {
        match rpc_client::get_transaction_receipt(rpc_url, hash).await {
            Ok(receipt) => {
                result.receipt = receipt;
            }
            Err(err) => {
                log::info!("Request error: {err}.");
            }
        }
    }
}

fn create_output_file(file_path: &Path) -> anyhow::Result<File> {
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .write(true)
        .open(file_path)?;

    Ok(file)
}
