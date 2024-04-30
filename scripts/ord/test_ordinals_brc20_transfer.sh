#!/usr/bin/env sh

# Transfer BRC20 tokens from `WALLET_ADDRESS` to `TO_ADDRESS`.
# `WALLET_ADDRESS` and `TO_ADDRESS` env is required.
# Related JSON data with token transfer params(for inscription) is stored in `ord-test-infra/brc20_transfer.json`.
# You can generate sample `TO_ADDRESS` with `test_ordinals_brc20_address.sh` script.

bitcoin_cli="docker exec bitcoind bitcoin-cli -regtest"
ord_wallet="docker exec ord ./ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet --server-url http://localhost:8000"

IID=$($ord_wallet inscribe --fee-rate 1 --file /brc20_json_inscriptions/brc20_transfer.json --destination $WALLET_ADDRESS | jq -r '.inscriptions[0].id')
$bitcoin_cli generatetoaddress 1 $WALLET_ADDRESS
$ord_wallet send --fee-rate 1 $TO_ADDRESS $IID
$bitcoin_cli generatetoaddress 1 $WALLET_ADDRESS
$bitcoin_cli getblockcount
