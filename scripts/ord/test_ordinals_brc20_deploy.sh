#!/usr/bin/env sh

# Deploy sample BRC20 token.
# `WALLET_ADDRESS` env is required.
# Related JSON data with token deploy params(for inscription) is stored in `ord-test-infra/brc20_deploy.json`.

docker exec -it ord-cli ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet inscribe --fee-rate 1 --file /brc20_json_inscriptions/brc20_deploy.json
docker exec -it bitcoind bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
docker exec -it bitcoind bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest getblockcount
