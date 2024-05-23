# This script tests deployment and bridging flow of the BTC Runes.
#
# To set up bitcoind and ord services before this script is run:
#
# > cd btc-deploy
# > docker compose down -v
# > docker compose up
#
# Then etch the test rune:
#
# > ./scripts/rune/etch.sh
#
# To make dfx trust local https certificate use mkcert:
#
# > export CAROOT=$PWD/btc-deploy/mkcert
# > mkcert install
#

set -e

CHAIN_ID=355113

ORD_DATA=$PWD/target/ord
RUNE_NAME="SUPERMAXRUNENAME"
RUNE_ID=$(ord -r --data-dir $ORD_DATA --index-runes runes | jq -r .runes.SUPERMAXRUNENAME.id)
rune_id_arr=(${RUNE_ID//:/ })
RUNE_BLOCK=${rune_id_arr[0]}
RUNE_TX_ID=${rune_id_arr[1]}

echo "Found rune id: ${RUNE_BLOCK}:${RUNE_TX_ID}"

INDEXER_URL="https://127.0.0.1:8001"

######### Start dfx #############

dfx stop
rm -f dfx_stderr.log
dfx start --background --clean --enable-bitcoin 2> dfx_stderr.log

dfx identity new --force btc-admin
dfx identity use btc-admin

ADMIN_PRINCIPAL=$(dfx identity get-principal)
ADMIN_WALLET=$(dfx identity get-wallet)


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
  --token-id="$RUNE_ID" \
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

if ! command -v bitcoin-cli &> /dev/null
then
  if command -v bitcoin-core.cli &> /dev/null
  then
    bc="bitcoin-core.cli -conf=$PWD/btc-deploy/bitcoin.conf -rpcwallet=admin"
    echo "Using bitcoin-core.cli as bitcoin-cli"
  else
    echo "bitcoin-cli could not be found"
    exit 1
  fi
else
  bc="bitcoin-cli -conf=$PWD/btc-deploy/bitcoin.conf -rpcwallet=admin"
fi

export ORD_BITCOIN_RPC_USERNAME=ic-btc-integration
export ORD_BITCOIN_RPC_PASSWORD="QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E="

ordw="ord -r --data-dir $ORD_DATA --index-runes wallet --server-url http://localhost:8000"

deposit_addr_resp=$(dfx canister call rune-bridge get_deposit_address "(\"$ETH_WALLET_ADDRESS\")")
res=${deposit_addr_resp#*\"}
DEPOSIT_ADDRESS=${res%\"*}

sleep 5

echo "Deposit address: $DEPOSIT_ADDRESS"

$ordw send --fee-rate 10 $DEPOSIT_ADDRESS 10:$RUNE_NAME
$bc generatetoaddress 1 bcrt1q7xzw9nzmsvwnvfrx6vaq5npkssqdylczjk8cts
sleep 5

$ordw send --fee-rate 10 $DEPOSIT_ADDRESS "0.0049 btc"
$bc generatetoaddress 1 bcrt1q7xzw9nzmsvwnvfrx6vaq5npkssqdylczjk8cts

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

receiver_resp=$($ordw receive)
RECEIVER=$(echo $receiver_resp | jq .addresses[0])
RECEIVER=$(echo $RECEIVER | tr -d '"')

echo "Ord balance before burn:"
$ordw balance

echo "Runes withdrawal receiver: $RECEIVER"

cargo run -q -p create_bft_bridge_tool -- burn-wrapped \
  --wallet="$ETH_WALLET" \
  --evm-canister="$EVM" \
  --bft-bridge="$BFT_ETH_ADDRESS" \
  --token-address="$TOKEN_ETH_ADDRESS" \
  --address="$RECEIVER" \
  --amount=10

echo "Wait for 15 seconds for the transaction to be broadcast"
sleep 15
$bc generatetoaddress 1 bcrt1q7xzw9nzmsvwnvfrx6vaq5npkssqdylczjk8cts

sleep 5
echo "Ord balance after burn:"
$ordw balance
