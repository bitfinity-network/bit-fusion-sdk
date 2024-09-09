#!/usr/bin/env sh

set -e
set -x

export RUST_BACKTRACE=full

# Configuration variables
WASM_DIR=".artifact"

# This is the hash of a recent commit on the https://github.com/dfinity/ic repository.
# It is used to identify the IC canisters to download.
# To be updated periodically to use the latest version.
IC_COMMIT_HASH="85bd56a70e55b2cea75cae6405ae11243e5fdad8" # 2024-02-21
EVM_FEATURES="export-api"

# Function to print help instructions
print_help() {
    echo "Usage: $0 [all|icrc2-bridge|rune-bridge|btc-bridge|erc20-bridge]"
    echo "Examples:"
    echo "  $0                          # Build all canisters, download binaries and build tools (default)"
    echo "  $0 all                      # Build all canisters and download binaries and build tools"
    echo "  $0 icrc2-bridge rune-bridge # Build the icrc2 and rune bridge"
}

# Initial setup
initialize_env() {
    [ -n "$ETHEREUM_GENESIS_ACCOUNTS" ] &&
        [ "$ETHEREUM_GENESIS_ACCOUNTS" -gt 0 ] &&
        EVM_FEATURES="$EVM_FEATURES,ethereum-genesis-accounts"

    echo "IC_HASH: $IC_HASH"

    if [ ! -f "./Cargo.toml" ]; then
        echo "Expecting to run from the cargo root directory, current directory is: $(pwd)"
        exit 42
    fi

    if [ "$CI" != "true" ]; then
        script_dir=$(dirname $0)
        project_dir=$(realpath "${script_dir}/..")

        echo "Project dir: \"$project_dir\""
        cd "$project_dir"

        rm -rf "$WASM_DIR"
        mkdir -p "$WASM_DIR"
    fi
}

# Function to download files
download_file() {
    local url="$1"
    local output_path="$2"
    echo "Downloading $url to $output_path"
    curl --create-dirs --fail -o "$output_path" "$url"
}

get_icrc1_binaries() {
    download_file "https://download.dfinity.systems/ic/${IC_COMMIT_HASH}/canisters/ic-icrc1-ledger.wasm.gz" "$WASM_DIR/icrc1-ledger.wasm.gz"
    download_file "https://raw.githubusercontent.com/dfinity/ic/${IC_COMMIT_HASH}/rs/rosetta-api/icrc1/ledger/ledger.did" "$WASM_DIR/icrc1.did"
}

get_ckbtc_binaries() {
    download_file "https://download.dfinity.systems/ic/${IC_COMMIT_HASH}/canisters/ic-ckbtc-minter.wasm.gz" "$WASM_DIR/ic-ckbtc-minter.wasm.gz"
    download_file "https://raw.githubusercontent.com/dfinity/ic/${IC_COMMIT_HASH}/rs/bitcoin/ckbtc/minter/ckbtc_minter.did" "$WASM_DIR/ic-ckbtc-minter.did"
    download_file "https://download.dfinity.systems/ic/${IC_COMMIT_HASH}/canisters/ic-ckbtc-kyt.wasm.gz" "$WASM_DIR/ic-ckbtc-kyt.wasm.gz"
    download_file "https://raw.githubusercontent.com/dfinity/ic/${IC_COMMIT_HASH}/rs/bitcoin/ckbtc/kyt/kyt.did" "$WASM_DIR/ic-ckbtc-kyt.did"

    cp src/integration-tests/ic-bitcoin-canister-mock.wasm.gz $WASM_DIR/ic-bitcoin-canister-mock.wasm.gz
}

build_bridge_tool() {
    echo "Building create BFTBridge tool"

    cargo build -p bridge-tool --release
    cp target/release/bridge-tool "$WASM_DIR/bridge-tool"
}

build_bridge_deployer_tool() {
    echo "Building create BFTBridge tool"

    cargo build -p bridge-deployer --release
    cp target/release/bridge-deployer "$WASM_DIR/bridge-deployer"
}

# Function to build a single canister with a feature flag
build_canister() {
    local canister_name="$1"
    local features="$2"
    local output_wasm="$3"
    local did_file_name="${4:-$canister_name}"

    mkdir -p "$WASM_DIR"

    # Generate the did file
#    cargo run -p "$canister_name" --features "$features" >"$WASM_DIR/$did_file_name.did"

    echo "Building $canister_name Canister with features: $features"

    cargo build --target wasm32-unknown-unknown --release --package "$canister_name" --features "$features"
    ic-wasm "target/wasm32-unknown-unknown/release/$canister_name.wasm" -o "$WASM_DIR/$output_wasm" shrink

    candid-extractor "$WASM_DIR/$output_wasm" > "$WASM_DIR/$did_file_name.did"

    gzip -k "$WASM_DIR/$output_wasm" --force
}

# Function to determine which canisters to build based on input
build_requested_canisters() {
    if [ $# -eq 0 ]; then
        set -- "all"

    elif [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
        print_help
        exit 0
    fi

    if [ "$1" = "all" ]; then
        initialize_env
        # Download binaries only if "all" is specified
        echo "Getting ICRC-1 Binaries"
        get_icrc1_binaries

        echo "Getting ckBTC Binaries"
        get_ckbtc_binaries

        # Build all canisters

        script_dir=$(dirname $0)
        project_dir=$(realpath "${script_dir}/..")

        build_canister "icrc2-bridge" "export-api" "icrc2-bridge.wasm" "icrc2-bridge"
        build_canister "erc20-bridge" "export-api" "erc20-bridge.wasm" "erc20-bridge"
        build_canister "brc20-bridge" "export-api" "brc20-bridge.wasm" "brc20-bridge"
        build_canister "btc-bridge" "export-api" "btc-bridge.wasm" "btc-bridge"
        build_canister "rune-bridge" "export-api" "rune-bridge.wasm" "rune-bridge"

        # Build tools
        build_bridge_tool
        build_bridge_deployer_tool
    else
        for canister in "$@"; do
            case "$canister" in
            evm)
                build_canister "evm_canister" "$EVM_FEATURES" "evm.wasm" "evm"
                ;;
            evm_testnet)
                build_canister "evm_canister" "$EVM_FEATURES,testnet" "evm_testnet.wasm" "evm_testnet"
                ;;
            signature_verification)
                build_canister "${canister}_canister" "export-api" "${canister}.wasm" "${canister}"
                ;;
            brc20-bridge | btc-bridge | rune-bridge | icrc2-bridge | erc20-bridge)
                build_canister "${canister}" "export-api" "${canister}.wasm" "${canister}"
                ;;
            *)
                echo "Error: Unknown canister '$canister'."
                print_help
                exit 1
                ;;
            esac
        done
    fi
}

main() {
    build_requested_canisters "$@"
}

main "$@"
