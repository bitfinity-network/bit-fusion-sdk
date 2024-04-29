#!/usr/bin/env sh

# Creates wallet and returns new address(other scripts require this address in `WALLET_ADDRESS` env, be sure to set with `export WALLET_ADDRESS=<ADDRESS>`).
# Also this script mine 101 blocks to activate wallet.

# docker exec ord wallet create
WALLET_ADDRESS=$(docker exec -it ord ./ord -r --index-runes --bitcoin-rpc-url=http://bitcoind:18443 wallet --server-url http://ord:8000 receive)
echo "$WALLET_ADDRESS"

WALLET_ADDRESS=$(docker exec -it ord wallet receive | jq -r ".addresses[0]")
docker exec -it bitcoind bitcoin-cli -regtest generatetoaddress 101 $WALLET_ADDRESS
echo $WALLET_ADDRESS
