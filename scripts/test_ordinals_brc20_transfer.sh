#!/usr/bin/env sh

# Transfer BRC20 tokens from `WALLET_ADDRESS` to `TO_ADDRESS`.
# `WALLET_ADDRESS` and `TO_ADDRESS` env is required.
# Related JSON data with token transfer params(for inscription) is stored in `ord-test-infra/brc20_transfer.json`.
# You can generate sample `TO_ADDRESS` with `test_ordinals_brc20_address.sh` script.

IID=$(ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet inscribe --fee-rate 1 --file ord-test-infra/brc20_json_inscriptions/brc20_transfer.json --destination $WALLET_ADDRESS | jq -r ".inscriptions[0].id")
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet send --fee-rate 1 $TO_ADDRESS $IID
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest getblockcount
