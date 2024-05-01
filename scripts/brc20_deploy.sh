#!/bin/bash

# Script to deploy and test the Brc20Bridge canister to perform a complete bridging flow (BRC20 <> ERC20).
#
# NOTES: 
#
# 1. Version 0.18 of dfx has a bug not allowing BTC operations to work properly. Future versions may fix the issue.
# Until then, this script uses dfx version 0.17.
#
# 2. We use https://127.0.0.1:8001 as the indexer to fetch a reveal transaction by its ID, and to fetch BRC20 inscription details.
# Notice it's using HTTPS, which is required by `dfx`. Therefore we need to set up two things:
#
#     a. `mkcert`, which automatically creates and installs a local CA in the system root store, and generates locally-trusted certificates.
#         After installing `mkcert` and generating the cert and key, you'll see an output like this:
#             The certificate is at "./localhost+2.pem" and the key at "./localhost+2-key.pem"
#
#     b. an SSL proxy (e.g. `local-ssl-proxy`, `caddy`, etc.) which utilises the cert and key generated previously:
#           local-ssl-proxy --source 8001 --target 8000 -c localhost+2.pem -k localhost+2-key.pem &
#

set +e

############################### Configure Dfx #################################

# echo "Starting dfx in a clean state"
# dfx stop
# rm -f dfx_log.txt
# dfx start --clean --background --enable-bitcoin  --host 127.0.0.1:4943 >dfx_log.txt 2>&1

dfx identity new --force brc20-admin
dfx identity use brc20-admin

######################### Deploy EVM and BRC20 Bridge ######################

ADMIN_PRINCIPAL=$(dfx identity get-principal)
ADMIN_WALLET=$(dfx identity get-wallet)
CHAIN_ID=355113
INDEXER_URL="https://127.0.0.1:9001"

echo "Deploying EVMc testnet"
dfx canister create evm_testnet
EVM=$(dfx canister id evm_testnet)

dfx deploy signature_verification --argument "(vec { principal \"${EVM}\" })"
SIGNATURE_VERIFICATION=$(dfx canister id signature_verification)

dfx deploy evm_testnet --argument "(record {
    min_gas_price = 10;
    signature_verification_principal = principal \"${SIGNATURE_VERIFICATION}\";
    log_settings = opt record {
      enable_console = true;
      in_memory_records = opt 10000;
      log_filter = opt \"info,brc20_bridge::scheduler=warn\";
    };
    owner = principal \"${ADMIN_PRINCIPAL}\";
    genesis_accounts = vec { };
    chain_id = $CHAIN_ID;
    coinbase = \"0x0000000000000000000000000000000000000000\";
})"

echo "Deploying BRC20 bridge"
dfx deploy brc20-bridge --argument "(record {
    indexer = \"${INDEXER_URL}\";
    erc20_minter_fee = 10;
    admin = principal \"${ADMIN_PRINCIPAL}\";
    signing_strategy = variant { ManagementCanister = record { key_id = variant { Dfx } } };
    evm_link = variant { Ic = principal \"${EVM}\" };
    network = variant { regtest };
    logger = record {
      enable_console = true;
      in_memory_records = opt 10000;
      log_filter = opt \"info\";
    };
})"

######################## Deploy BFT and Token Contracts ######################

ETH_WALLET=$(cargo run -q -p create_bft_bridge_tool -- create-wallet --evm-canister="$EVM")
ETH_WALLET_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET")
ETH_WALLET_CANDID=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET" --candid)

BRC20_BRIDGE=$(dfx canister id brc20-bridge)

res=$(dfx canister call brc20-bridge get_evm_address)
res=${res#*\"}
BRC20_BRIDGE_ETH_ADDRESS=${res%\"*}

echo "BRC20 bridge eth address: ${BRC20_BRIDGE_ETH_ADDRESS}"

echo "Minting ETH tokens for BRC20 bridge canister"
dfx canister call evm_testnet mint_native_tokens "(\"${BRC20_BRIDGE_ETH_ADDRESS}\", \"340282366920938463463374607431768211455\")"

BFT_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- deploy-bft-bridge --minter-address="$BRC20_BRIDGE_ETH_ADDRESS" --evm="$EVM" --wallet="$ETH_WALLET")
echo "BFT ETH address: $BFT_ETH_ADDRESS"

TOKEN_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- create-token \
  --bft-bridge-address="$BFT_ETH_ADDRESS" \
  --token-name=KOBP \
  --token-id="$BRC20_BRIDGE" \
  --evm-canister="$EVM" \
  --wallet="$ETH_WALLET")

echo "Wrapped token ETH address: $TOKEN_ETH_ADDRESS"

echo "Configuring BRC20 bridge canister"
dfx canister call brc20-bridge admin_configure_bft_bridge "(record {
  decimals = 0;
  token_symbol = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
  token_address = \"$TOKEN_ETH_ADDRESS\";
  bridge_address = \"$BFT_ETH_ADDRESS\";
  erc20_chain_id = 355113;
  token_name = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
})"

echo "All canisters successfully deployed."

######################## Prepare Inscription Addresses ######################

bitcoin_cli="docker exec bitcoind bitcoin-cli -regtest"
ord_wallet="docker exec ord ./ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet --server-url http://localhost:8000"

wallet_exists=$($bitcoin_cli listwallets | grep -c "admin")

if [ "$wallet_exists" -eq 0 ]; then
    echo "Creating 'admin' wallet"
    $bitcoin_cli createwallet "admin"
fi

echo "Loading 'admin' wallet"
load_output=$($bitcoin_cli loadwallet "admin" 2>&1)

if echo "$load_output" | grep -q "Wallet file verification failed"; then
    echo "Wallet already exists but could not be loaded due to verification failure."
elif echo "$load_output" | grep -q "Wallet loaded successfully"; then
    echo "Wallet loaded successfully."
else
    echo "Unexpected wallet load output: $load_output"
fi

ADMIN_ADDRESS=$($bitcoin_cli -rpcwallet=admin getnewaddress)
echo "Admin address: $ADMIN_ADDRESS"

# Generate 101 blocks to ensure enough coins are available for spending
height=$($bitcoin_cli getblockcount)
if [ "$height" -lt 101 ]; then
    echo "Generating 101 blocks..."
    $bitcoin_cli generatetoaddress 101 "$ADMIN_ADDRESS"
fi

sleep 5

ORD_ADDRESS=$($ord_wallet receive | jq -r .addresses[0])
echo "Ord wallet address: $ORD_ADDRESS"

$bitcoin_cli -rpcwallet=admin sendtoaddress "$ORD_ADDRESS" 10
$bitcoin_cli -rpcwallet=admin generatetoaddress 1 "$ADMIN_ADDRESS"

##################### Create a BRC20 Inscription ###################

echo "Creating a BRC20 inscription"
sleep 5
inscription_res=$($ord_wallet inscribe --fee-rate 10 --file /brc20_json_inscriptions/brc20_deploy.json)

sleep 1
$bitcoin_cli -rpcwallet=admin generatetoaddress 10 "$ADMIN_ADDRESS"

sleep 5
$bitcoin_cli -rpcwallet=admin generatetoaddress 1 "$ADMIN_ADDRESS"

sleep 3
BRC20_ID=$(echo "$inscription_res" | jq -r '.inscriptions[0].id')
echo "BRC20 inscription ID: $BRC20_ID"

sleep 3
$ord_wallet balance

####################### Swap BRC20 for ERC20 ######################

echo "Preparing to bridge a BRC20 inscription to an ERC20 token"

brc20_bridge_addr=$(dfx canister call brc20-bridge get_deposit_address)
BRIDGE_ADDRESS=$(echo "$brc20_bridge_addr" | sed -e 's/.*"\(.*\)".*/\1/')
echo "BRC20 bridge canister BTC address: $BRIDGE_ADDRESS"

echo "Topping up canister's wallet"
docker exec bitcoind bitcoin-cli -regtest generatetoaddress 10 "$BRIDGE_ADDRESS"

sleep 10
echo "Canister's balance after topup"
dfx canister call brc20-bridge get_balance "(\"$BRIDGE_ADDRESS\")"

echo "Ord wallet balance before deposit of BRC20"
$ord_wallet balance

# Deposit BRC20 on the bridge
$ord_wallet send --fee-rate 10 $BRIDGE_ADDRESS $BRC20_ID
$bitcoin_cli generatetoaddress 1 "$ORD_ADDRESS"

sleep 10
echo "Ord wallet balance after deposit"
$ord_wallet balance

echo "Canister's balance after BRC20 deposit"
dfx canister call brc20-bridge get_balance "(\"$BRIDGE_ADDRESS\")"

BRC20_TICKER="kobp"

for i in 1 2 3; do
  sleep 5
  echo "Trying to bridge from BRC20 to ERC20"
  mint_status=$(dfx canister call brc20-bridge brc20_to_erc20 "(\"$BRC20_TICKER\", \"$BRIDGE_ADDRESS\", \"$ETH_WALLET_ADDRESS\")")
  echo "Result: $mint_status"

  if [[ $mint_status == *"Minted"* ]]; then
    echo "Minting of ERC20 token successful."
    break
  fi

  if [[ $i -eq 3 ]]; then
    echo "Failed to mint after 3 retries"
    exit 1
  fi
done

sleep 5

####################### Swap ERC20 for BRC20 ######################

# USER_ADDRESS=$($ord_wallet receive | jq -r .addresses[0])
# echo "Inscription destination and leftovers address: $USER_ADDRESS"

# echo "Ord wallet balance before swap:"
# $ord_wallet balance

# cargo run -q -p create_bft_bridge_tool -- burn-wrapped \
#   --wallet="$ETH_WALLET" \
#   --evm-canister="$EVM" \
#   --bft-bridge="$BFT_ETH_ADDRESS" \
#   --token-address="$TOKEN_ETH_ADDRESS" \
#   --address="$USER_ADDRESS" \
#   --amount=10

# echo "Wait for 15 seconds for the transaction to be broadcast"
# sleep 15
# $bitcoin_cli generatetoaddress 1 "$ORD_ADDRESS"

# sleep 5
# echo "Ord wallet balance after swap:"
# $ord_wallet balance
