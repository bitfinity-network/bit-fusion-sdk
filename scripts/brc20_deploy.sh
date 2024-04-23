#!/bin/bash

# Script to deploy the Brc20Bridge canister to perform a complete bridging flow (BRC20 <> ERC20).
#
# NOTE: Version 0.18 of dfx has a bug not allowing BTC operations to work properly. Future versions may fix the issue.
# Until then, this script uses dfx version 0.17.
set +e

CHAIN_ID=355113

GENERAL_INDEXER_URL="https://blockstream.info"
ORDINALS_INDEXER_URL="https://api.hiro.so/ordinals/v1/brc-20/tokens"

RPC_URL=${"http://127.0.0.1:18444"}
RPC_USER=${"icp"}
RPC_PASSWORD=${"test"}

dfx identity new --force brc20-admin
dfx identity use brc20-admin

ADMIN_PRINCIPAL=$(dfx identity get-principal)
ADMIN_WALLET=$(dfx identity get-wallet)

######################### Deploy EVM and BRC20 Bridge ######################

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
        log_filter = null;
    };
    owner = principal \"${ADMIN_PRINCIPAL}\";
    genesis_accounts = vec { };
    chain_id = $CHAIN_ID;
    coinbase = \"0x0000000000000000000000000000000000000000\";
})"
# log_filter = opt \"error,did=debug,evm_core=debug,evm=debug\";

echo "Deploying BRC20 bridge"
dfx deploy brc20-bridge --argument "(record {
    general_indexer = \"${GENERAL_INDEXER_URL}\";
    erc20_minter_fee = 10;
    admin = principal \"${ADMIN_PRINCIPAL}\";
    signing_strategy = variant { ManagementCanister = record { key_id = variant { Dfx } } };
    evm_link = variant { Ic = principal \"${EVM}\" };
    network = variant { regtest };
    regtest_rpc = record {
        url = \"${RPC_URL}\";
        user = \"${RPC_USER}\";
        password = \"${RPC_PASSWORD}\";
    };
    logger = record {
       enable_console = true;
       in_memory_records = opt 10000;
       log_filter = opt \"info\";
    };
    ordinals_indexer = \"${ORDINALS_INDEXER_URL}\";
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
  --token-name=BRC20 \
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

######################## Swap BRC20 for ERC20 ######################

echo "Bridging a BRC20 inscription to an ERC20 token"

# 1. Get deposit address
brc20_bridge_addr=$(dfx canister call brc20-bridge get_deposit_address "(\"$ETH_WALLET_ADDRESS\")")
addr=${brc20_bridge_addr#*\"}
DEPOSIT_ADDRESS=${addr%\"*}
echo "BRC20 deposit address: $DEPOSIT_ADDRESS"

# 2. Top up canister's wallet
CONTAINER_ID=$(docker ps -q --filter "name=bitcoind")
docker exec -it $CONTAINER_ID bitcoin-cli -regtest generatetoaddress 1 $DEPOSIT_ADDRESS

# 3. Send a BRC20 inscription to the deposit address
TX_IDS=$(dfx canister call brc20-bridge inscribe "(variant { Brc20 },
  \"{\\"p\\": \\"brc-20\\",\\"op\\":\\"deploy\\",\\"tick\\":\\"kobp\\",\\"max\\":\\"1000\\",\\"lim\\":\\"10\\",\\"dec\\":\\"8\\"}\",
  \"$DEPOSIT_ADDRESS\",
  \"$DEPOSIT_ADDRESS\",
  null
)")

echo "Inscription transaction IDs: $TX_IDS"

# 4. Swap the BRC20 inscription for an ERC20 token
for i in 1 2 3
do
  sleep 5
  echo "Trying to deposit"
  mint_status=$(dfx canister call brc20-bridge brc20_to_erc20 "(\"kobp\", \"$DEPOSIT_ADDRESS\", \"$ETH_WALLET_ADDRESS\")")
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
