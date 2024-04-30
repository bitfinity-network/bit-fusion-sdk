#!/usr/bin/env sh

# Creates wallet and returns new address(other scripts require this address in `WALLET_ADDRESS` env, be sure to set with `export WALLET_ADDRESS=<ADDRESS>`).
# Also this script mine 101 blocks to activate wallet.

bitcoin_cli="docker exec bitcoind bitcoin-cli -regtest"
ord_wallet="docker exec ord ./ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet --server-url http://localhost:8000"

$ord_wallet create
WALLET_ADDRESS=$($ord_wallet receive | jq -r '.addresses[0]')
$bitcoin_cli generatetoaddress 101 $WALLET_ADDRESS
echo $WALLET_ADDRESS
