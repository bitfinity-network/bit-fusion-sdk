#!/bin/bash
set -e

WASM_DIR=.artifact

create() {
    # Create canisters
    NETWORK=$1

    dfx canister --network=$NETWORK create --with-cycles=600000000000 --all
}

deploy() {
    set -e

    NETWORK=$1
    INSTALL_MODE=$2
    LOG_SETTINGS=$3
    OWNER=$4
    CHAIN_ID=$5

    dfx build --network=$NETWORK

    if [ "$NETWORK" = "local" ]; then
        deploy_icrc1_canister "$NETWORK" "$INSTALL_MODE" "$OWNER"
        token_id=$(dfx canister --network=$NETWORK id token)
    fi

    # Get EVM canister ID
    evm_id=$(dfx canister --network=$NETWORK id evm_testnet)

    spender_id=$(dfx canister --network=$NETWORK id spender)

    deploy_minter_canister "$NETWORK" "$INSTALL_MODE" "$evm_id" "$CHAIN_ID" "$OWNER" "$LOG_SETTINGS" "$spender_id"

    minter_id=$(dfx canister --network=$NETWORK id minter)

    deploy_spender_canister "$NETWORK" "$INSTALL_MODE" "$minter_id"

    deploy_signature_verification_canister "$NETWORK" "$INSTALL_MODE" "$evm_id"

    signature_verification_id=$(dfx canister --network=$NETWORK id signature_verification)

    # Prepare a transaction to create a bridge contract
    minter_address=$(get_minter_address "$NETWORK")
    deploy_bft_contract_tx=$(get_bft_contract_tx "$minter_address" "$CHAIN_ID")
    deploy_bft_contract_sender=$(get_tx_sender "$deploy_bft_contract_tx")

    deploy_evm_canister "$NETWORK" "$INSTALL_MODE" "$LOG_SETTINGS" "$minter_address" "$deploy_bft_contract_sender" "$signature_verification_id" "$OWNER" "$CHAIN_ID"

    get_bft_bridge_contract_response=$(dfx canister --network=$NETWORK call minter get_bft_bridge_contract)
    if [[ $get_bft_bridge_contract_response == "(variant { Ok = null })" ]]; then
        deploy_bridge_contract "$deploy_bft_contract_tx" "$NETWORK" "$CHAIN_ID"
    fi

    echo "Token ($token_id), Minter ($minter_id), EVM ($evm_id), and Signature Verification ($signature_verification_id) canisters initialized."

}

deploy_minter_canister() {
    echo "Deploying minter canister"
    NETWORK=$1
    INSTALL_MODE=$2
    EVM_ID=$3
    EVM_CHAIN_ID=$4
    OWNER=$5
    LOG_SETTINGS=$6
    SPENDER_ID=$7

    # Check if network is ic or local
    if [ "$NETWORK" = "ic" ]; then
        signing_strategy="(variant { ManagementCanister= record {
            key_id= variant { Production };
            derivation_path= vec {};
        } })"
    else
        signing_strategy="(variant { Local= record { private_key= vec { 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; 1; }; } })"
    fi

    minter_init_args="(record {
        owner=principal \"$OWNER\";
        evm_principal=principal \"$EVM_ID\";
        evm_chain_id=$EVM_CHAIN_ID;
        evm_gas_price=\"0xa\";
        signing_strategy=$signing_strategy;
        initial_native_tokens_count=\"0x7fffffffffffffffffffffffffffffff\";
        new_pair_fee=\"0x0\";
        process_transactions_results_interval= opt record {secs= 1; nanos= 0;};
        spender_principal=principal \"$SPENDER_ID\";
        log_settings=$LOG_SETTINGS;
    })"

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK --argument="$minter_init_args" minter

}

deploy_spender_canister() {
    echo "Deploying spender canister"
    NETWORK=$1
    INSTALL_MODE=$2
    MINTER_CANISTER=$3

    spender_args="(principal \"$MINTER_CANISTER\")"

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK --argument="$spender_args" spender
}

# Extract the address from the response
extract_address() {
    echo $1 | sed -n -e 's/^.*"\(0x[a-fA-F0-9]*\)".*$/\1/p'
}

# Get the minter address
get_minter_address() {
    set -e
    NETWORK=$1
    MINTER_ID=$(dfx canister --network=$NETWORK id minter)

    MINTER_ADDRESS=$(dfx canister --network=$NETWORK call minter get_minter_canister_evm_address "()")
    MINTER_ADDRESS=$(extract_address "$MINTER_ADDRESS")
    echo $MINTER_ADDRESS
}

# Create a Candid text format encoded transaction that creates the bridge contract
get_bft_contract_tx() {
    MINTER_ADDRESS=$1
    CHAIN_ID=$2

    SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
    CREATE_BFT_BRIDGE_TOOL="$SCRIPT_DIR/../../.artifact/create_bft_bridge_tool"

    # In CI it happens to be not executable
    [ -x "$CREATE_BFT_BRIDGE_TOOL" ] || chmod +x "$CREATE_BFT_BRIDGE_TOOL"

    $CREATE_BFT_BRIDGE_TOOL --minter-address "$MINTER_ADDRESS" --chain-id "$CHAIN_ID"
}

deploy_evm_canister() {
    NETWORK=$1
    INSTALL_MODE=$2
    LOG_SETTINGS=$3
    MINTER_ADDRESS=$4
    CONTRACT_SENDER=$5
    SIGNATURE_VERIFICATION=$6
    OWNER=$7
    CHAIN_ID=$8

    # Init EVM canister with token canister ID as argument
    # Note that for the contract sender we set the balance enough to create the contract and that's it
    evm_init_args="(record {
        min_gas_price= 10:nat;
        signature_verification_principal=principal \"$SIGNATURE_VERIFICATION\";
        log_settings=$LOG_SETTINGS;
        owner=principal \"$OWNER\";
        genesis_accounts=vec { record { 0= \"$MINTER_ADDRESS\"}; record { 0= \"$CONTRACT_SENDER\"; 1= opt \"0x20000000000000\"} };
        chain_id = $CHAIN_ID:nat64;
        coinbase = \"0x0000000000000000000000000000000000000000\";
    })"

    echo "Deploying EVM canister with init args: $evm_init_args"

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK --argument="$evm_init_args" evm_testnet
}

# Get sender address from the Candid-encoded transaction
get_tx_sender() {
    TX=$1
    echo $TX | sed -n -e 's/^.*from = "\(0x[a-fA-F0-9]*\)".*$/\1/p'
}

deploy_bridge_contract() {
    CREATE_BRIDGE_TX=$1
    NETWORK=$2
    CHAIN_ID=$3

    # Deploy the bridge contract on the EVM
    TX_HASH=$(dfx canister --network=$NETWORK call evm_testnet send_raw_transaction "$CREATE_BRIDGE_TX")
    TX_HASH=$(extract_address "$TX_HASH")

    sleep 5

    # Get contract address
    TX_RECEIPT=$(dfx canister --network=$NETWORK call evm_testnet eth_get_transaction_receipt "(\"$TX_HASH\")")
    CONTRACT_ADDRESS=$(echo "$TX_RECEIPT" | sed -n -e 's/^.*contractAddress = opt "\(0x[a-fA-F0-9]*\)".*$/\1/p')

    # Set bridge contract address to the minter canister
    dfx canister --network=$NETWORK call minter register_evmc_bft_bridge "(\"$CONTRACT_ADDRESS\")"
}

deploy_icrc1_canister() {
    NETWORK=$1
    INSTALL_MODE=$2
    OWNER=$3

    token_init_args="(variant {Init = record {
        token_symbol = \"TOKEN\";
        token_name = \"TKN\";
        minting_account = record { owner = principal \"$OWNER\" };
        transfer_fee = 1000;
        metadata = vec {};
        initial_balances = vec {
            record {
              record { owner = principal \"$OWNER\" };
              1_000_000_000_000;
          }
        };
        archive_options = record {
            num_blocks_to_archive = 2000;
            trigger_threshold = 1000;
            controller_id = principal \"$OWNER\";
        };
        feature_flags =  opt record { icrc2 = true};
        }})"

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK token --argument="$token_init_args"
}

deploy_signature_verification_canister() {
    NETWORK=$1
    INSTALL_MODE=$2
    EVM_ID=$3

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK signature_verification --argument="(vec { principal \"$EVM_ID\" })"
}
