#!/usr/bin/env sh

set -e
set -x

export RUST_BACKTRACE=full

# Configuration variables
WASM_DIR=".artifact"

ORD_FEATURES="export-api"

# Initial setup
initialize_env() {
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

# Function to build a single canister with a feature flag
build_canister() {
    local canister_name="$1"
    local features="$2"
    local did_file_name="${3:-$canister_name}"

    echo "Building $canister_name Canister with features: $features"

    # Check for macOS-specific requirements before building
    if [ "$(uname)" == "Darwin" ]; then
        LLVM_PATH=$(brew --prefix llvm)
        # On macOS, use the brew versions of llvm-ar and clang
        AR="${LLVM_PATH}/bin/llvm-ar" CC="${LLVM_PATH}/bin/clang" cargo build --target wasm32-unknown-unknown --release --package "$canister_name" --features "$features"
    else
        cargo build --target wasm32-unknown-unknown --release --package "$canister_name" --features "$features"
    fi

    ic-wasm "target/wasm32-unknown-unknown/release/$canister_name.wasm" -o "$WASM_DIR/$canister_name.wasm" shrink
    gzip -k "$WASM_DIR/$canister_name.wasm" --force
    cargo run -p "$canister_name" --features "$features" >"$WASM_DIR/$did_file_name.did"
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

        # Build all canisters

        script_dir=$(dirname $0)
        project_dir=$(realpath "${script_dir}/..")

        build_canister "inscriber" "$ORD_FEATURES"
    else
        for canister in "$@"; do
            case "$canister" in
            inscriber)
                build_canister "inscriber" "$ORD_FEATURES"
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
