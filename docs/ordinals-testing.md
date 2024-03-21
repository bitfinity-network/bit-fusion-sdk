# Ordinals Testing
## Overview
All infrastructure and config files for local testing of ordinals(BRC20, etc) is located in the `ord-testnet` folder. It is supplied as a set of docker compose services without any additional setup/installation scripts.

## Run local testing infrastructure
The following steps will help you get started with testing:
1. ```cd ord-testnet```
2. ```docker compose up```
3. ```dfx start --clean --enable-bitcoin```

## Docker compose services
- Postgres instance which beign used by indexer and api to store / query / parse inscriptions. Exposes `5432` port by default.
- Bitcoin regtest node with almost default configuration. Exposes `18443`, `18444` RPC ports and `28332`, `28333` for ZeroMQ.
- Ordinals API which receive raw transactions / blocks data from indexer, parses them and store in postgres database. Exposes `3000` port.
- Ordhook indexer get updates(transactions, blocks) from `bitcoin` service via ZeroMQ. Indexer parse inscriptions and push them into ordinals api.
- Ordinals Explorer available on [http://localhost:1337](http://localhost:1337) and provides friendly UI with ordinals data. Allows you to view any inscriptions and BRC20 transactions.
- Ord-cli provides ord tool which helps to create inscriptions in a simple way. See available scripts below.
- Ord additional indexer(server) on port `1338` to observe ordinal runes.

## Scripts
Example scripts provided in `scripts/ord/test_ordinals_*.sh` folder:
- `test_ordinals_init.sh` - Creates wallet and returns new address(other scripts require this address in `WALLET_ADDRESS` env, be sure to set with `export WALLET_ADDRESS=<ADDRESS>`). Also this script mine 101 blocks to activate wallet.
- `test_ordinals_brc20_deploy.sh` - Deploy sample BRC20 token. `WALLET_ADDRESS` env is required. Related JSON data with token params(for inscription) is stored in `ord-testnet/brc20_deploy.json`.
- `test_ordinals_brc20_mint.sh` - Mint BRC20 tokens. `WALLET_ADDRESS` env is required. Related JSON data with token mint params(for inscription) is stored in `ord-testnet/brc20_mint.json`.
- `test_ordinals_brc20_transfer.sh` - Transfer BRC20 tokens. `WALLET_ADDRESS` and `TO_ADDRESS` env is required. Related JSON data with token transfer params(for inscription) is stored in `ord-testnet/brc20_transfer.json`. You can generate sample `TO_ADDRESS` with `test_ordinals_brc20_address.sh` script.
- `test_ordinals_mine.sh` - Generates bitcoins for specific address(mining).

## Example with Rust and canisters
You can refer to [ckOrd](https://github.com/bitfinity-network/ckOrd) canister for more detailed infrastructure usage and tests. This canister provides inscriber functionality for executing bitcoin ordinal inscriptions.
