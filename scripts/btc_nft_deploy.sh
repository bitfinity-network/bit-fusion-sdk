#!/bin/bash

set +e

echo "Starting dfx in a clean state"
dfx stop
rm -f dfx_log.txt
dfx start --clean --background --enable-bitcoin  --host 127.0.0.1:4943 >dfx_log.txt 2>&1

dfx identity new --force btc-admin
dfx identity use btc-admin

######################### Deploy EVM and NFT Bridge ######################

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
      log_filter = opt \"info,nft_bridge::scheduler=warn\";
    };
    owner = principal \"${ADMIN_PRINCIPAL}\";
    genesis_accounts = vec { };
    chain_id = $CHAIN_ID;
    coinbase = \"0x0000000000000000000000000000000000000000\";
})"

echo "Deploying BTC NFT bridge"
dfx deploy btc-nft-bridge --argument "(record {
    indexer = \"${INDEXER_URL}\";
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

NFT_BRIDGE=$(dfx canister id btc-nft-bridge)

res=$(dfx canister call btc-nft-bridge get_evm_address)
res=${res#*\"}
NFT_BRIDGE_ETH_ADDRESS=${res%\"*}

echo "NFT bridge eth address: ${NFT_BRIDGE_ETH_ADDRESS}"

echo "Minting ETH tokens for NFT bridge canister"
dfx canister call evm_testnet mint_native_tokens "(\"${NFT_BRIDGE_ETH_ADDRESS}\", \"340282366920938463463374607431768211455\")"

ERC721_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- deploy-erc721-bridge --minter-address="$NFT_BRIDGE_ETH_ADDRESS" --evm="$EVM" --wallet="$ETH_WALLET")
echo "NFT ETH address: $ERC721_ETH_ADDRESS"

TOKEN_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- create-nft \
  --erc721-bridge-address="$ERC721_ETH_ADDRESS" \
  --token-name=KOBP \
  --token-id="$NFT_BRIDGE" \
  --evm-canister="$EVM" \
  --wallet="$ETH_WALLET")

echo "Wrapped token ETH address: $TOKEN_ETH_ADDRESS"

echo "Configuring NFT bridge canister"
dfx canister call btc-nft-bridge admin_configure_nft_bridge "(record {
  token_symbol = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
  token_address = \"$TOKEN_ETH_ADDRESS\";
  bridge_address = \"$ERC721_ETH_ADDRESS\";
  erc721_chain_id = 355113;
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

echo "Creating a NFT inscription"
sleep 5
inscription_res=$($ord_wallet inscribe --fee-rate 10 --file /nft_json_inscriptions/demo.json)

sleep 1
$bitcoin_cli -rpcwallet=admin generatetoaddress 10 "$ADMIN_ADDRESS"

sleep 5
$bitcoin_cli -rpcwallet=admin generatetoaddress 1 "$ADMIN_ADDRESS"

sleep 3
NFT_ID=$(echo "$inscription_res" | jq -r '.inscriptions[0].id')
echo "NFT inscription ID: $NFT_ID"

sleep 3
$ord_wallet balance

####################### Swap NFT for ERC721 ######################

echo "Preparing to bridge a NFT inscription to an ERC721 token"

nft_bridge_addr=$(dfx canister call btc-nft-bridge get_deposit_address)
BRIDGE_ADDRESS=$(echo "$nft_bridge_addr" | sed -e 's/.*"\(.*\)".*/\1/')
echo "BRC20 bridge canister BTC address: $BRIDGE_ADDRESS"

echo "Topping up canister's wallet"
docker exec bitcoind bitcoin-cli -regtest generatetoaddress 10 "$BRIDGE_ADDRESS"

sleep 10
echo "Canister's balance after topup"
dfx canister call brc20-bridge get_balance "(\"$BRIDGE_ADDRESS\")"

echo "Ord wallet balance before deposit of BRC20"
$ord_wallet balance

# Deposit NFT on the bridge
$ord_wallet send --fee-rate 10 $BRIDGE_ADDRESS $BRC20_ID
$bitcoin_cli generatetoaddress 1 "$ORD_ADDRESS"

sleep 10
echo "Ord wallet balance after deposit"
$ord_wallet balance

echo "Canister's balance after NFT deposit"
dfx canister call brc20-bridge get_balance "(\"$BRIDGE_ADDRESS\")"

for i in 1 2 3; do
  sleep 5
  echo "Trying to bridge from BTC-NFT to ERC721"
  mint_status=$(dfx canister call brc20-bridge nft_to_erc721 "(\"$NFT_ID\", \"$BRIDGE_ADDRESS\", \"$ETH_WALLET_ADDRESS\")")
  echo "Result: $mint_status"

  if [[ $mint_status == *"Minted"* ]]; then
    echo "Minting of ERC721 token successful."
    break
  fi

  if [[ $i -eq 3 ]]; then
    echo "Failed to mint after 3 retries"
    exit 1
  fi
done

sleep 5
