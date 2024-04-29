#!/usr/bin/env sh

# Deploy sample BRC20 token.
# `WALLET_ADDRESS` env is required.
# Related JSON data with token deploy params(for inscription) is stored in `ord-test-infra/brc20_deploy.json`.
export WALLET_ADDRESS="bcrt1pcjgrns6mzwenc6cgqnhcqxp9v33f9mx4wj5j5tq695umkt0mu9jsj5apt2"

docker exec -it ord ./ord -r --index-runes --bitcoin-rpc-url=http://bitcoind:18443 wallet --server-url http://ord:8000 inscribe --fee-rate 1 --file /brc20_json_inscriptions/brc20_deploy.json
docker exec -it bitcoind bitcoin-cli -regtest generatetoaddress 1 $WALLET_ADDRESS
docker exec -it bitcoind bitcoin-cli -regtest getblockcount
docker exec -it ord ./ord -r --index-runes --bitcoin-rpc-url=http://bitcoind:18443 wallet --server-url http://ord:8000 inscriptions
