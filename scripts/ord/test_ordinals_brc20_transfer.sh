#!/usr/bin/env sh

# Transfer BRC20 tokens from `WALLET_ADDRESS` to `TO_ADDRESS`.
# `WALLET_ADDRESS` and `TO_ADDRESS` env is required.
# Related JSON data with token transfer params(for inscription) is stored in `ord-test-infra/brc20_transfer.json`.
# You can generate sample `TO_ADDRESS` with `test_ordinals_brc20_address.sh` script.

IID=$(docker exec -it ord-cli ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet inscribe --fee-rate 1 --file /brc20_json_inscriptions/brc20_transfer.json --destination $WALLET_ADDRESS | jq -r ".inscriptions[0].id")
docker exec -it bitcoind bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
docker exec -it ord-cli ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet send --fee-rate 1 $TO_ADDRESS $IID
docker exec -it bitcoind bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
docker exec -it bitcoind bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest getblockcount
