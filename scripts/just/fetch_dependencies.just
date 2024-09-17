# This is the hash of a recent commit on the https://github.com/dfinity/ic repository.
# It is used to identify the IC canisters to download.
# To be updated periodically to use the latest version.
# This commit is from: 2024-02-21
IC_COMMIT_HASH:="85bd56a70e55b2cea75cae6405ae11243e5fdad8"


# Fetch all dependencies
[group('fetch_dependencies')]
fetch_all_dependencies: fetch_ic_icrc1_ledger_canister fetch_ic_ckbtc_minter_canister


# Fetch the icrc1-ledger canister wasm and did files
[group('fetch_dependencies')]
fetch_ic_icrc1_ledger_canister:
  just download_file "https://download.dfinity.systems/ic/{{IC_COMMIT_HASH}}/canisters/ic-icrc1-ledger.wasm.gz" "{{WASM_DIR}}/icrc1-ledger.wasm.gz"
  just download_file "https://raw.githubusercontent.com/dfinity/ic/{{IC_COMMIT_HASH}}/rs/rosetta-api/icrc1/ledger/ledger.did" "{{WASM_DIR}}/icrc1.did"


# Fetch the ic-ckbtc-minter and kyt canisters wasm and did files
[group('fetch_dependencies')]
fetch_ic_ckbtc_minter_canister:
  just download_file "https://download.dfinity.systems/ic/{{IC_COMMIT_HASH}}/canisters/ic-ckbtc-minter.wasm.gz" "{{WASM_DIR}}/ic-ckbtc-minter.wasm.gz"
  just download_file "https://raw.githubusercontent.com/dfinity/ic/{{IC_COMMIT_HASH}}/rs/bitcoin/ckbtc/minter/ckbtc_minter.did" "{{WASM_DIR}}/ic-ckbtc-minter.did"
  just download_file "https://download.dfinity.systems/ic/{{IC_COMMIT_HASH}}/canisters/ic-ckbtc-kyt.wasm.gz" "{{WASM_DIR}}/ic-ckbtc-kyt.wasm.gz"
  just download_file "https://raw.githubusercontent.com/dfinity/ic/{{IC_COMMIT_HASH}}/rs/bitcoin/ckbtc/kyt/kyt.did" "{{WASM_DIR}}/ic-ckbtc-kyt.did"

  cp src/integration-tests/ic-bitcoin-canister-mock.wasm.gz {{WASM_DIR}}/ic-bitcoin-canister-mock.wasm.gz


# Function to download files
[private]
download_file url output_path:
    echo "Downloading {{url}} to {{output_path}}"
    curl --create-dirs --fail -o "{{output_path}}" "{{url}}"