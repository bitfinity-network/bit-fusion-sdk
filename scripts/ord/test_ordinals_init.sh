#!/usr/bin/env sh

# Creates wallet and returns new address(other scripts require this address in `WALLET_ADDRESS` env, be sure to set with `export WALLET_ADDRESS=<ADDRESS>`).
# Also this script mine 101 blocks to activate wallet.

docker exec ord-cli ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet --server-url http://ord:1338 create
WALLET_ADDRESS=$(docker exec -it ord-cli ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet --server-url http://ord:1338 receive | jq -r ".addresses[0]")
docker exec -it bitcoind bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 101 $WALLET_ADDRESS
echo $WALLET_ADDRESS
