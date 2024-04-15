
# bitcoin-core.daemon -conf=${PWD}/src/create_bft_bridge_tool/bitcoin.conf -datadir=${PWD}/target/bc -txindex -fallbackfee=0.000001
# ord -r --bitcoin-rpc-username ic-btc-integration --bitcoin-rpc-password QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E= --index-runes --data-dir target/bc server --http-port 8000
# local-ssl-proxy --source 8001 --target 8000 -c localhost+1.pem -k localhost+1-key.pem
#
# ./scripts/build.sh rune-bridge
# ./scripts/rune_deploy.sh
# ord -r --bitcoin-rpc-username ic-btc-integration --bitcoin-rpc-password QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E= --index-runes --data-dir target/bc wallet --server-url http://0.0.0.0:8000 send --fee-rate 10 bcrt1qc440vkzfe8evdpgv40fhl88el27zjdk42nvl9e 10:SUPERMAXRUNENAME
# bitcoin-core.cli -conf="${PWD}/src/create_bft_bridge_tool/bitcoin.conf" generatetoaddress 1 bcrt1q7xzw9nzmsvwnvfrx6vaq5npkssqdylczjk8cts
# dfx canister call rune-bridge deposit '("0xc4a06e28a173fc74b668d0819b6ca656500e4f37")'




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
    rune_name = \"$RUNE_NAME\";
    indexer_url = \"$INDEXER_URL\";
    deposit_fee = 100_000;
})"

dfx canister call rune-bridge admin_configure_ecdsa

########## Deploy BFT and Token contracts ##########

ETH_WALLET=$(cargo run -q -p create_bft_bridge_tool -- create-wallet --evm-canister="$EVM")
ETH_WALLET_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET")
ETH_WALLET_CANDID=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET" --candid)

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

DEPOSIT_ADDRESS=$(dfx canister call rune-bridge get_deposit_address "(\"$ETH_WALLET_ADDRESS\")")
echo "Deposit address: $DEPOSIT_ADDRESS"


