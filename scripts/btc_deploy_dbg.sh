# Script to set up BTC bridge infrasturcture into local DFX replica and test BTC bridging flow
# It uses ./dfx instead of dfx command as the current version of dfx (0.18) has a bug not allowing BTC operations to
# work. To run the script download the dfx v0.17 from https://github.com/dfinity/sdk/releases and put it into the root
# of the repo.
#
# For btc operations it uses bitcoin core. In Ubuntu this tool is bitcoin-core.cli and bitcoin-core.daemon, but on
# other platforms it can be bitcoind and bitcoin.cli. Adjust accordingly. Before the script is run, the daemon must
# be run:
# bitcoin-core.daemon -conf=<path_to_config> -datadir=<path_to_data_dir>
set -e

CHAIN_ID=355113

echo "" > dfx_stderr.log
./dfx stop
./dfx start --background --clean --enable-bitcoin 2> dfx_stderr.log

./dfx identity new --force btc-admin
./dfx identity use btc-admin

ADMIN_PRINCIPAL=$(./dfx identity get-principal)
ADMIN_WALLET=$(./dfx identity get-wallet)

########## Deploy ckBTC canisters ##########

./dfx canister create token
./dfx canister create ic-ckbtc-kyt
./dfx canister create ic-ckbtc-minter

CKBTC_LEDGER=$(./dfx canister id token)
CKBTC_KYT=$(./dfx canister id ic-ckbtc-kyt)
CKBTC_MINTER=$(./dfx canister id ic-ckbtc-minter)

./dfx deploy token --argument "(variant {Init = record {
    minting_account = record { owner = principal \"$CKBTC_MINTER\" };
    transfer_fee = 10;
    token_symbol = \"ckTESTBTC\";
    token_name = \"Chain key testnet Bitcoin\";
    metadata = vec {};
    initial_balances = vec {};
    max_memo_length = opt 100;
    archive_options = record {
        num_blocks_to_archive = 1000;
        trigger_threshold = 2000;
        max_message_size_bytes = null;
        cycles_for_archive_creation = opt 1_000_000_000_000;
        node_max_memory_size_bytes = opt 3_221_225_472;
        controller_id = principal \"$ADMIN_WALLET\"
    }
}})"

./dfx deploy ic-ckbtc-kyt --argument "(variant {InitArg = record {
    api_key = \"abcdef\";
    maintainers = vec { principal \"$ADMIN_PRINCIPAL\"; };
    mode = variant { AcceptAll };
    minter_id = principal \"$CKBTC_MINTER\";
} })"

./dfx canister call ic-ckbtc-kyt set_api_key "(record { api_key = \"abc\"; })"

./dfx deploy ic-ckbtc-minter --argument "(variant {Init = record {
    btc_network = variant { Regtest };
    ledger_id = principal \"$CKBTC_LEDGER\";
    ecdsa_key_name = \"dfx_test_key\";
    retrieve_btc_min_amount = 5_000;
    max_time_in_queue_nanos = 420_000_000_000;
    min_confirmations = opt 1;
    kyt_fee = opt 1000;
    kyt_principal = opt principal \"$CKBTC_KYT\";
    mode = variant { GeneralAvailability };
}})"

########## Deploy EVM and BTC bridge ##########

./dfx canister create evm_testnet
EVM=$(./dfx canister id evm_testnet)

./dfx deploy signature_verification --argument "(vec { principal \"${EVM}\" })"
SIGNATURE_VERIFICATION=$(./dfx canister id signature_verification)

./dfx deploy evm_testnet --argument "(record {
    min_gas_price = 10;
    signature_verification_principal = principal \"${SIGNATURE_VERIFICATION}\";
    log_settings = opt record {
        enable_console = true;
        in_memory_records = opt 10000;
        log_filter = opt \"error,did=debug,evm_core=debug,evm=debug\";
    };
    owner = principal \"${ADMIN_PRINCIPAL}\";
    genesis_accounts = vec { };
    chain_id = $CHAIN_ID;
    coinbase = \"0x0000000000000000000000000000000000000000\";
})"

./dfx deploy btc-bridge --argument "(record {
    admin = principal \"${ADMIN_PRINCIPAL}\";
    signing_strategy = variant {
       Local = record {
           private_key = blob \"\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\67\\01\";
       }
    };
    ck_btc_ledger_fee = 10;
    evm_link = variant { Ic = principal \"${EVM}\" };
    ck_btc_minter = principal \"${CKBTC_MINTER}\";
    network = variant { regtest };
    ck_btc_ledger = principal \"${CKBTC_LEDGER}\";
    log_settings = record {
       enable_console = true;
       in_memory_records = opt 10000;
       log_filter = opt \"trace\";
    };
})"

########## Deploy BFT and Token contracts ##########

ETH_WALLET=$(cargo run -q -p create_bft_bridge_tool -- create-wallet --evm-canister="$EVM")
ETH_WALLET_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET")

echo "ETH wallet key: ${ETH_WALLET}"
echo "ETH wallet address: ${ETH_WALLET_ADDRESS}"

ETH_WALLET_CANDID=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET" --candid)

BTC_BRIDGE=$(./dfx canister id btc-bridge)

res=$(./dfx canister call btc-bridge get_evm_address)
res=${res#*\"}
BTC_BRIDGE_ETH_ADDRESS=${res%\"*}

echo "BTC bridge eth address: ${BTC_BRIDGE_ETH_ADDRESS}"

echo "Minting ETH tokens for BTC bridge canister"
./dfx canister call evm_testnet mint_native_tokens "(\"${BTC_BRIDGE_ETH_ADDRESS}\", \"340282366920938463463374607431768211455\")"

BFT_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- deploy-bft-bridge --minter-address="$BTC_BRIDGE_ETH_ADDRESS" --evm="$EVM" --wallet="$ETH_WALLET")
echo "BFT ETH address: $BFT_ETH_ADDRESS"

TOKEN_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- create-token \
  --bft-bridge-address="$BFT_ETH_ADDRESS" \
  --token-name=BTC \
  --token-id="$CKBTC_LEDGER" \
  --evm-canister="$EVM" \
  --wallet="$ETH_WALLET")

echo "Wrapped token ETH address: $TOKEN_ETH_ADDRESS"

echo "Configuring BTC bridge canister"
./dfx canister call btc-bridge admin_configure_bft_bridge "(record {
  decimals = 0;
  token_symbol = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
  token_address = \"$TOKEN_ETH_ADDRESS\";
  bridge_address = \"$BFT_ETH_ADDRESS\";
  erc20_chain_id = 355113;
  token_name = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
})"

########## Create BTC and move them into wrapped EVM token ##########

r1=$(./dfx canister call ic-ckbtc-minter get_btc_address "(record { owner = opt principal \"$BTC_BRIDGE\"; subaccount = opt $ETH_WALLET_CANDID; })")
r2=${r1#*\"}
address=${r2%\"*}

echo Deposit BTC address: "$address"

echo "Minting BTC block to ckBTC minter deposit address"
./bitcoin/bin/bitcoin-cli -conf="${PWD}/src/create_bft_bridge_tool/bitcoin.conf" generatetoaddress 1 "$address"

# Wait to let ckBTC get the info about the new block
sleep 10

echo "Requesting minting wrapped tokens"
./dfx canister call btc-bridge btc_to_erc20 "\"$ETH_WALLET_ADDRESS\""

# Wait for EVM to process the mint transaction
sleep 5

########## Burn wrapped token and receive BTC to the current wallet ##########

BTC_ADDRESS=$(./bitcoin/bin/bitcoin-cli -conf="${PWD}/src/create_bft_bridge_tool/bitcoin.conf" getnewaddress)

echo "wallet: $ETH_WALLET"
echo "evm-canister: $EVM"
echo "bft-bridge: $BFT_ETH_ADDRESS"
echo "token-address: $TOKEN_ETH_ADDRESS"
echo "address bitcoin-cli getnewaddress: $BTC_ADDRESS"

cargo run -q -p create_bft_bridge_tool -- burn-wrapped \
  --wallet="$ETH_WALLET" \
  --evm-canister="$EVM" \
  --bft-bridge="$BFT_ETH_ADDRESS" \
  --token-address="$TOKEN_ETH_ADDRESS" \
  --address="$BTC_ADDRESS" \
  --amount=100000000

sleep 5
./dfx canister call ic-ckbtc-minter retrieve_btc_status_v2_by_account "(opt record { owner = principal \"$BTC_BRIDGE\"; })"
