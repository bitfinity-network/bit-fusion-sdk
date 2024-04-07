# Script to set up BTC bridge infrasturcture into local DFX replica and test BTC bridging flow
# It uses dfx instead of dfx command as the current version of dfx (0.18) has a bug not allowing BTC operations to
# work. To run the script download the dfx v0.17 from https://github.com/dfinity/sdk/releases and put it into the root
# of the repo.
#
# For btc operations it uses bitcoin core. In Ubuntu this tool is ~/bitcoin-25.0/bin/bitcoin-cli and bitcoin-core.daemon, but on
# other platforms it can be bitcoind and bitcoin.cli. Adjust accordingly. Before the script is run, the daemon must
# be run:
# bitcoin-core.daemon -conf=<path_to_config> -datadir=<path_to_data_dir>
set -e

start_icx() {
    killall icx-proxy
    sleep 2
    # Start ICX Proxy
    dfx_local_port=$(dfx info replica-port)
    icx-proxy --fetch-root-key --address 127.0.0.1:8545 --dns-alias 127.0.0.1:$EVM --replica http://localhost:$dfx_local_port &
    sleep 2

    curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc": "2.0", "method": "eth_chainId", "params": [], "id":1}' 'http://127.0.0.1:8545'
}

CHAIN_ID=355113

dfx stop
dfx start --host 127.0.0.1:4943 --background --clean --enable-bitcoin 2> dfx_stderr.log

dfx identity new --force btc-admin
dfx identity use btc-admin

ADMIN_PRINCIPAL=$(dfx identity get-principal)
ADMIN_WALLET=$(dfx identity get-wallet)

########## Deploy ckBTC canisters ##########

dfx canister create token2

CKBTC_LEDGER=$(dfx canister id token)
CKBTC_KYT=$(dfx canister id ic-ckbtc-kyt)
CKBTC_MINTER=$(dfx canister id ic-ckbtc-minter)
ICRC2_TOKEN=$(dfx canister id token2)

USER_PRINCIPICAL="qhjy5-udjmu-rqh6d-abbcl-63l4x-gbwoi-ip5qu-vocyn-docsu-fideu-eae"
dfx deploy token2 --argument "(variant {Init = record {
    minting_account = record { owner = principal \"$ADMIN_PRINCIPAL\" };
    transfer_fee = 10;
    token_symbol = \"AUX\";
    token_name = \"Aux Token\";
    metadata = vec {};
    initial_balances = vec {                                
        record {                                            
            record {                                        
                owner = principal \"$USER_PRINCIPICAL\";   
                subaccount = null;                          
            };                                              
            100_000_000_000                                 
        }                                                   
    };  
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

########## Deploy EVM, BTC bridge, ICRC2 Minter ##########

dfx canister create evm_testnet
dfx canister create icrc2-minter
dfx canister create spender
EVM=$(dfx canister id evm_testnet)
ICRC2_MINTER=$(dfx canister id icrc2-minter)
SPENDER=$(dfx canister id spender)

dfx deploy signature_verification --argument "(vec { principal \"${EVM}\" })"
SIGNATURE_VERIFICATION=$(dfx canister id signature_verification)

dfx deploy evm_testnet --argument "(record {
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

dfx deploy icrc2-minter  --argument "(record {
    evm_principal = principal \"$EVM\";
    signing_strategy = variant { 
        ManagementCanister = record {
            key_id = variant { Dfx };
        }
    };
    owner = principal \"$ADMIN_PRINCIPAL\";
    spender_principal = principal \"$SPENDER\";
})"

dfx deploy spender --argument "(principal \"$ICRC2_MINTER\")"

start_icx

########## Deploy BFT and Token contracts ##########

ETH_WALLET=$(cargo run -q -p create_bft_bridge_tool -- create-wallet --evm-canister="$EVM")
ETH_WALLET_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET")
ETH_WALLET_CANDID=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET" --candid)

BTC_BRIDGE=$(dfx canister id btc-bridge)

res=$(dfx canister call btc-bridge get_evm_address)
res=${res#*\"}
BTC_BRIDGE_ECDSA_ADDRESS=${res%\"*}

echo "BTC bridge ecdsa address: ${BTC_BRIDGE_ECDSA_ADDRESS}"

echo "Minting ETH tokens for BTC bridge canister"
dfx canister call evm_testnet mint_native_tokens "(\"${BTC_BRIDGE_ECDSA_ADDRESS}\", \"340282366920938463463374607431768211455\")"

BTC_BRIDGE_CONTRACT_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- deploy-bft-bridge --minter-address="$BTC_BRIDGE_ECDSA_ADDRESS" --evm="$EVM" --wallet="$ETH_WALLET")
echo "BTC bridge contract address: $BTC_BRIDGE_CONTRACT_ADDRESS"

TOKEN_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- create-token \
  --bft-bridge-address="$BTC_BRIDGE_CONTRACT_ADDRESS" \
  --token-name=BTC \
  --token-id="$CKBTC_LEDGER" \
  --evm-canister="$EVM" \
  --wallet="$ETH_WALLET")

echo "Wrapped token ETH address: $TOKEN_ETH_ADDRESS" 

echo "Configuring BTC bridge canister"
dfx canister call btc-bridge admin_configure_bft_bridge "(record {
  decimals = 0;
  token_symbol = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
  token_address = \"$TOKEN_ETH_ADDRESS\";
  bridge_address = \"$BTC_BRIDGE_CONTRACT_ADDRESS\";
  erc20_chain_id = 355113;
  token_name = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
})"

######### Deploy BFT Bridge and Token Contract for ICRC2 Minter ###########

res=$(dfx canister call icrc2-minter get_minter_canister_evm_address)
res=${res#*\"}
ICRC2_MINTER_ECDSA_ADDRESS=${res%\"*}

echo "ICRC2 Minter ecdsa address: ${ICRC2_MINTER_ECDSA_ADDRESS}"

echo "Minting ETH tokens for ICRC2 Minter canister"
dfx canister call evm_testnet mint_native_tokens "(\"${ICRC2_MINTER_ECDSA_ADDRESS}\", \"340282366920938463463374607431768211455\")"

ICRC2_BRIDGE_CONTRACT_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- deploy-bft-bridge --minter-address="$ICRC2_MINTER_ECDSA_ADDRESS" --evm="$EVM" --wallet="$ETH_WALLET")
echo "ICRC2 bridge contract address: $ICRC2_BRIDGE_CONTRACT_ADDRESS"

echo "Register bft bridge contract address with icrc2-minter"
dfx canister call icrc2-minter register_evmc_bft_bridge "(\"$ICRC2_BRIDGE_CONTRACT_ADDRESS\")"

ICRC2_WRAPPED_TOKEN_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- create-token \
  --bft-bridge-address="$ICRC2_BRIDGE_CONTRACT_ADDRESS" \
  --token-name="Aux Token" \
  --token-id="$ICRC2_TOKEN" \
  --evm-canister="$EVM" \
  --wallet="$ETH_WALLET")

echo "ICRC2 Wrapped token address: $ICRC2_WRAPPED_TOKEN_ADDRESS"

########## Create BTC and move them into wrapped EVM token ##########

r1=$(dfx canister call ic-ckbtc-minter get_btc_address "(record { owner = opt principal \"$BTC_BRIDGE\"; subaccount = opt $ETH_WALLET_CANDID; })")
r2=${r1#*\"}
address=${r2%\"*}

echo Deposit BTC address: "$address"

echo "Minting BTC block to ckBTC minter deposit address"
~/bitcoin-25.0/bin/bitcoin-cli -conf="${PWD}/src/create_bft_bridge_tool/bitcoin.conf" generatetoaddress 1 "$address"

# Wait to let ckBTC get the info about the new block
sleep 10

echo "Requesting minting wrapped tokens"
dfx canister call btc-bridge btc_to_erc20 "\"$ETH_WALLET_ADDRESS\""

# Wait for EVM to process the mint transaction
sleep 5

########## Burn wrapped token and receive BTC to the current wallet ##########

BTC_ADDRESS=$(~/bitcoin-25.0/bin/bitcoin-cli -conf="${PWD}/src/create_bft_bridge_tool/bitcoin.conf" getnewaddress)

cargo run -q -p create_bft_bridge_tool -- burn-wrapped \
  --wallet="$ETH_WALLET" \
  --evm-canister="$EVM" \
  --bft-bridge="$BTC_BRIDGE_CONTRACT_ADDRESS" \
  --token-address="$TOKEN_ETH_ADDRESS" \
  --address="$BTC_ADDRESS" \
  --amount=100000000

sleep 5
dfx canister call ic-ckbtc-minter retrieve_btc_status_v2_by_account "(opt record { owner = principal \"$BTC_BRIDGE\"; })"
