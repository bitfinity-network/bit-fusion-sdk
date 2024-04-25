# This script tests deployment and bridging flow of the BTC Runes. The script uses `bitcoin-core.daemon` and
# `bitcoin-core.cli` applications for bitcoin operations. Depending on your installation you may need to change these
# to `bitcoind` and `bitcoind.cli`.
#
# Before the script is run, a few services are needed to be started:
#
# 1. Bitcoin daemon with transaction index.
#
# bitcoin-core.daemon -conf=${PWD}/src/create_bft_bridge_tool/bitcoin.conf -datadir=${PWD}/target/bc -txindex -fallbackfee=0.000001
#
# 2. Ord indexer with support for runes.

# ord -r --bitcoin-rpc-username ic-btc-integration --bitcoin-rpc-password QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E= --index-runes --data-dir target/bc server --http-port 8000
#
# 3. Https proxy to let dfx connect to the indexer. You can use the proxy you like. For example, `local-ssl-proxy`:
#
# local-ssl-proxy --source 8001 --target 8000 -c localhost+1.pem -k localhost+1-key.pem
#
# Not though that for the https certificats to be accepted by dfx, a local CA authority must be installed in the system
# and the certificates (`.pem` files above) must be created by that authority. You can use `mkcert` tool for that.
#
# 4. Create the test rune with
#
# ord -r --bitcoin-rpc-username ic-btc-integration --bitcoin-rpc-password QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E= --index-runes --data-dir target/bc wallet --server-url http://0.0.0.0:8000 batch --fee-rate 10 --batch ./scripts/rune/batch.yaml
#
# After this command is run it will wait for 10 blocks to be mined in BTC network. After that the rune is created. You
# need to modify `RUNE_BLOCK` and `RUNE_TX_ID` variables in the script below to match your values.

set -e

CHAIN_ID=355113

dfx stop
rm -f dfx_stderr.log
dfx start --background --clean --enable-bitcoin 2> dfx_stderr.log

dfx identity new --force btc-admin
dfx identity use btc-admin

ADMIN_PRINCIPAL=$(dfx identity get-principal)
ADMIN_WALLET=$(dfx identity get-wallet)

RUNE_NAME="SUPERMAXRUNENAME"
RUNE_BLOCK="113"
RUNE_TX_ID="1"

INDEXER_URL="https://127.0.0.1:8001"

########## Deploy EVM and Rune bridge ##########

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
        log_filter = opt \"warn\";
    };
    owner = principal \"${ADMIN_PRINCIPAL}\";
    genesis_accounts = vec { };
    chain_id = $CHAIN_ID;
    coinbase = \"0x0000000000000000000000000000000000000000\";
})"

dfx deploy rune-bridge --argument "(record {
    network = variant { regtest };
    evm_link = variant { Ic = principal \"${EVM}\" };
    signing_strategy = variant { ManagementCanister = record { key_id = variant { Dfx } } };
    admin = principal \"${ADMIN_PRINCIPAL}\";
    log_settings = record {
       enable_console = true;
       in_memory_records = opt 10000;
       log_filter = opt \"trace,rune_bridge::scheduler=warn\";
    };
    min_confirmations = 1;
    rune_info = record {
      name = \"$RUNE_NAME\";
      block = $RUNE_BLOCK;
      tx = $RUNE_TX_ID;
    };
    indexer_url = \"$INDEXER_URL\";
    deposit_fee = 100_000;
})"

dfx canister call rune-bridge admin_configure_ecdsa

########## Deploy BFT and Token contracts ##########

ETH_WALLET=$(cargo run -q -p create_bft_bridge_tool -- create-wallet --evm-canister="$EVM")
ETH_WALLET_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET")
ETH_WALLET_CANDID=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET" --candid)

echo "ETH wallet PK: $ETH_WALLET"

RUNE_BRIDGE=$(dfx canister id rune-bridge)

res=$(dfx canister call rune-bridge get_evm_address)
res=${res#*\"}
RUNE_BRIDGE_ETH_ADDRESS=${res%\"*}

echo "Rune bridge eth address: ${RUNE_BRIDGE_ETH_ADDRESS}"

echo "Minting ETH tokens for Rune bridge canister"
dfx canister call evm_testnet mint_native_tokens "(\"${RUNE_BRIDGE_ETH_ADDRESS}\", \"340282366920938463463374607431768211455\")"

BFT_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- deploy-bft-bridge --minter-address="$RUNE_BRIDGE_ETH_ADDRESS" --evm="$EVM" --wallet="$ETH_WALLET")
echo "BFT ETH address: $BFT_ETH_ADDRESS"

TOKEN_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- create-token \
  --bft-bridge-address="$BFT_ETH_ADDRESS" \
  --token-name=RUNE \
  --token-id="$RUNE_BRIDGE" \
  --evm-canister="$EVM" \
  --wallet="$ETH_WALLET")

echo "Wrapped token ETH address: $TOKEN_ETH_ADDRESS"

echo "Configuring Rune bridge canister"
dfx canister call rune-bridge admin_configure_bft_bridge "(record {
  decimals = 0;
  token_symbol = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
  token_address = \"$TOKEN_ETH_ADDRESS\";
  bridge_address = \"$BFT_ETH_ADDRESS\";
  erc20_chain_id = 355113;
  token_name = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
})"

########### Deposit runes ##########

deposit_addr_resp=$(dfx canister call rune-bridge get_deposit_address "(\"$ETH_WALLET_ADDRESS\")")
res=${deposit_addr_resp#*\"}
DEPOSIT_ADDRESS=${res%\"*}
echo "Deposit address: $DEPOSIT_ADDRESS"

ord -r --bitcoin-rpc-username ic-btc-integration --bitcoin-rpc-password QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E= \
  --index-runes --data-dir target/bc wallet --server-url http://0.0.0.0:8000 send --fee-rate 10 \
  $DEPOSIT_ADDRESS 10:$RUNE_NAME

bitcoin-core.cli -conf="${PWD}/src/create_bft_bridge_tool/bitcoin.conf" sendtoaddress $DEPOSIT_ADDRESS 0.0049
bitcoin-core.cli -conf="${PWD}/src/create_bft_bridge_tool/bitcoin.conf" generatetoaddress 1 bcrt1q7xzw9nzmsvwnvfrx6vaq5npkssqdylczjk8cts


for i in 1 2 3
do
  sleep 5
  echo "Trying to deposit"
  response=$(dfx canister call rune-bridge deposit "(\"$ETH_WALLET_ADDRESS\")")
  echo "Response: $response"

  if [[ $response == *"Minted"* ]]; then
    break
  fi

  if [[ i = 3 ]]; then
    return "Failed to mint after 3 retries"
  fi
done

sleep 5

receiver_resp=$(ord -r --bitcoin-rpc-username ic-btc-integration --bitcoin-rpc-password QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E= --index-runes --data-dir target/bc wallet --server-url http://0.0.0.0:8000 receive)
RECEIVER=$(echo $receiver_resp | jq .addresses[0])
RECEIVER=$(echo $RECEIVER | tr -d '"')

echo "Runes withdrawal receiver: $RECEIVER"

cargo run -q -p create_bft_bridge_tool -- burn-wrapped \
  --wallet="$ETH_WALLET" \
  --evm-canister="$EVM" \
  --bft-bridge="$BFT_ETH_ADDRESS" \
  --token-address="$TOKEN_ETH_ADDRESS" \
  --address="$RECEIVER" \
  --amount=10