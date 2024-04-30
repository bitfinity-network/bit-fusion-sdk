#!/usr/bin/env sh

# Mint BRC20 tokens to `WALLET_ADDRESS`.
# `WALLET_ADDRESS` env is required.
# Related JSON data with token mint params(for inscription) is stored in `ord-test-infra/brc20_mint.json`.

bitcoin_cli="docker exec bitcoind bitcoin-cli -regtest"
ord_wallet="docker exec ord ./ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet --server-url http://localhost:8000"

$ord_wallet inscribe --fee-rate 1 --file /brc20_json_inscriptions/brc20_mint.json --destination $WALLET_ADDRESS
$bitcoin_cli generatetoaddress 1 $WALLET_ADDRESS
$bitcoin_cli getblockcount
