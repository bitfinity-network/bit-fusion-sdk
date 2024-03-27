#!/usr/bin/env sh

# Mint BRC20 tokens to `WALLET_ADDRESS`.
# `WALLET_ADDRESS` env is required.
# Related JSON data with token mint params(for inscription) is stored in `ord-test-infra/brc20_mint.json`.

docker exec -it ord-cli ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet --server-url http://ord:1338 inscribe --fee-rate 1 --file /brc20_json_inscriptions/brc20_mint.json --destination $WALLET_ADDRESS
docker exec -it bitcoind bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
docker exec -it bitcoind bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest getblockcount
