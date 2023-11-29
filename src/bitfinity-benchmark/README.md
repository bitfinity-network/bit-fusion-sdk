# Bitfinity Benchmarking CLI

- [Bitfinity Benchmarking CLI](#bitfinity-benchmarking-cli)
  - [Purpose](#purpose)
  - [Terminology](#terminology)
  - [Random transactions](#random-transactions)
    - [Usage - Random transactions](#usage---random-transactions)
    - [Example - Random transactions](#example---random-transactions)
    - [Output - Random transactions](#output---random-transactions)
  - [Replicate Ethereum Mainnet](#replicate-ethereum-mainnet)
    - [Usage - Replicate Ethereum Mainnet](#usage---replicate-ethereum-mainnet)
    - [Example - Replicate Ethereum Mainnet](#example---replicate-ethereum-mainnet)
    - [Output - Replicate Ethereum Mainnet](#output---replicate-ethereum-mainnet)

A concurrent Bitfinity benchmarking Command Line Interface (CLI) for testing transaction throughput on Bitfinity network.

## Purpose

The purpose of this tool is to test the performance of the Bitfinity network and to find the optimal transaction throughput for the network.

## Terminology

- Stimulus: Transactions that are sent to a node in a period of time at some rate (transactions per second).

- Response: The receipts that are sent back to the source; in this test the time that each response takes to be done and how much receipts are sent from the Bitfinity network will be measured.

- Throughput: The number of transactions that are sent to the network in a period of time.

## Random transactions

This tool executes a specified amount of transactions against the EVM canister.

### Usage - Random transactions

You can build the tool from this repository.

```bash
cargo build --release  --bin random-transactions-benchmark
```

The binary will be located at `target/release/random-transactions-benchmark`.

```bash
$ ./target/release/random-transactions-benchmark --help

Tool for benchmarking BitFinity Network

Usage: random-transactions-benchmark [OPTIONS] --test-time <TEST_TIME> --url <RPC_URL> --output <OUTPUT> --rate <RATE> --fetch-receipts <FETCH_RECEIPTS>

Options:
  -t, --test-time <TEST_TIME>  Test time in seconds
  -r, --rate <RATE>            The rate of transactions to be sent per second. Default is 10
  -u, --url <RPC_URL>          The RPC URL to send transactions to
  -o, --output <OUTPUT>        The output folder to write the results to. Default is target
  -f, --fetch-receipts <FETCH_RECEIPTS>
                               Fetch receipts from the network. Default is false
  -h, --help                   Print help
  -V, --version                Print version

```

### Example - Random transactions

```bash
random-transactions-benchmark -t 10 -r 10 -u http://127.0.0.1:8545 -o ./data --fetch-receipts true
```

or:

```bash
cargo run --release --bin random-transactions-benchmark -- -t 10 -r 10 -u http://127.0.0.1:8545 -o ./data --fetch-receipts true
```

This command will send 100 transactions to the node in 10 seconds with a rate of 10 transactions per second.

### Output - Random transactions

You can also see the output of the tool in the terminal.

```bash
$ ./target/release/random-transactions-benchmark -t 10 -r 10 -u http://127.0.0.1:8545 -o ./data
[INFO  bitfinity_benchmark] BitFinity Benchmarking
[INFO  bitfinity_benchmark] ----------------------
[INFO  bitfinity_benchmark] - test-time: 10
[INFO  bitfinity_benchmark] - rate: 10
[INFO  bitfinity_benchmark] - rpc-url: "http://127.0.0.1:8545"
[INFO  bitfinity_benchmark] - output: "./data"
[INFO  bitfinity_benchmark] - fetch-receipts: false
[INFO  bitfinity_benchmark] ----------------------
[INFO  bitfinity_benchmark] Total Transactions: 100
[INFO  bitfinity_benchmark] Starting pre-benchmark phase
[INFO  bitfinity_benchmark] Pre-benchmark phase complete
[INFO  bitfinity_benchmark] Starting benchmarking
[INFO  bitfinity_benchmark::stats] Sending transactions at 10 txns/sec
[INFO  bitfinity_benchmark::stats] ----------------- Statistics ---------------
[INFO  bitfinity_benchmark::stats] Total Transactions Sent: 100
[INFO  bitfinity_benchmark::stats] Time(s) : 18.3
[INFO  bitfinity_benchmark::stats] Rate 5.5 tx/s
[INFO  bitfinity_benchmark::stats] ------------------------------------------------------
[INFO  bitfinity_benchmark] Benchmarking complete
```

The Tool generates a `csv` files in output passed through the CLI arguments and it contains the statistics of the test, both stimulus and response.
The file contains the following columns:

- `transaction_index`: The index of the transaction in the test.
- `start_time`: The time that the transaction was sent.
- `end_time`: The time that the transaction was received.

With these columns, you can construct a graph of the transactions that were sent and received in the test.

## Replicate Ethereum Mainnet

This tools replicate the entire Ethereum transactions against EVM canister.

### Usage - Replicate Ethereum Mainnet

You can build the tool from this repository.

```bash
cargo build --release --bin replicate-ethereum-mainnet-benchmark
```

The binary will be located at `target/release/replicate-ethereum-mainnet-benchmark`.

```bash
$ cargo run --bin replicate-ethereum-mainnet-benchmark -- --help

Tool for benchmarking BitFinity Network

Usage: replicate-ethereum-mainnet-benchmark [OPTIONS] --url <URL> --evmc <EVMC>

Options:
  -i, --identity <IDENTITY>
          Identity for Canister client. If not provided the canister metrics won't be collected
  -f, --fetch-receipts
          Fetch receipts for transactions - This will measure the time it takes for a transaction to be included in a block. Default is false
  -u, --url <URL>
          The RPC URL to send transactions to
      --evmc <EVMC>
          Evmc canister principal
      --ethereum-url <ETHEREUM_URL>
          Ethereum main net RPC URL [default: https://cloudflare-eth.com/]
  -o, --output <OUTPUT>
          The output file to write the results to
  -n, --network <NETWORK>
          [default: ic]
  -s, --start-block <START_BLOCK>
          block to start with ("0xb443" is the first block with a transaction in ethereum) [default: 0xb443]
  -I, --transaction-processing-interval <TRANSACTION_PROCESSING_INTERVAL>
          transaction processing interval (ms) [default: 1000]
  -h, --help
          Print help
  -V, --version
          Print version
```

### Example - Replicate Ethereum Mainnet

> ‼️ The first transaction is at block 0xb443

```bash
cargo run --bin replicate-ethereum-mainnet-benchmark -- --evmc emz6j-kiaaa-aaaak-ae35a-cai -u "https://emz6j-kiaaa-aaaak-ae35a-cai.raw.icp0.io" -o "/tmp/output.csv" --fetch-receipts --start-block 0xb443 -i <path_to_identity>
```

This command will start scraping block's transactions from block `0xb443` and will start executing them against EVM with principal `emz6j-kiaaa-aaaak-ae35a-cai` using `https://emz6j-kiaaa-aaaak-ae35a-cai.raw.icp0.io` as endpoint. It will output the results to `/tmp/output.csv` (the file won't be truncated).

### Output - Replicate Ethereum Mainnet

The Tool generates a `csv` files in output passed through the CLI arguments and it contains the statistics of the test, both stimulus and response.
The file contains the following columns:

- `hash`: Transaction hash.
- `status`
  - `NOK`
  - `OK`
  - `IGN` (if fetch_receipts is false)
- `start_time`: The time that the transaction was sent.
- `end_time`: The time that the transaction was received.
- `stable_memory_size`: stable memory size of evmc after tx execution
- `heap_memory_size`: heap memory size of evmc after tx execution

With these columns, you can construct a graph of the transactions that were sent and received in the test.
