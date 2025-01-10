# This is the hash of a recent commit on the https://github.com/dfinity/ic repository.
# It is used to identify the IC canisters to download.
# To be updated periodically to use the latest version.
# This commit is from: 2024-02-21
IC_COMMIT_HASH:="85bd56a70e55b2cea75cae6405ae11243e5fdad8"

# This is the dfinity evm-rpc-canister tag to download
IC_EVM_RPC_TAG:="release-2024-05-23"

# This is the bitfinity evmc tag to download
EVMC_TAG:="v0.2.0_2068bd15"


# Fetch all dependencies
[group('fetch_dependencies')]
fetch_all_canisters: fetch_evmc_canisters fetch_ic_canisters


# Fetch all required IC canister wasm and did files
[group('fetch_dependencies')]
fetch_ic_canisters: pre_build
  curl --fail -L -o "{{WASM_DIR}}/icrc1-ledger.wasm.gz"    "https://download.dfinity.systems/ic/{{IC_COMMIT_HASH}}/canisters/ic-icrc1-ledger.wasm.gz"
  curl --fail -L -o "{{WASM_DIR}}/icrc1-ledger.did"        "https://raw.githubusercontent.com/dfinity/ic/{{IC_COMMIT_HASH}}/rs/rosetta-api/icrc1/ledger/ledger.did" 
  curl --fail -L -o "{{WASM_DIR}}/ic-ckbtc-minter.wasm.gz" "https://download.dfinity.systems/ic/{{IC_COMMIT_HASH}}/canisters/ic-ckbtc-minter.wasm.gz" 
  curl --fail -L -o "{{WASM_DIR}}/ic-ckbtc-minter.did"     "https://raw.githubusercontent.com/dfinity/ic/{{IC_COMMIT_HASH}}/rs/bitcoin/ckbtc/minter/ckbtc_minter.did" 
  curl --fail -L -o "{{WASM_DIR}}/ic-ckbtc-kyt.wasm.gz"    "https://download.dfinity.systems/ic/{{IC_COMMIT_HASH}}/canisters/ic-ckbtc-kyt.wasm.gz" 
  curl --fail -L -o "{{WASM_DIR}}/ic-ckbtc-kyt.did"        "https://raw.githubusercontent.com/dfinity/ic/{{IC_COMMIT_HASH}}/rs/bitcoin/ckbtc/kyt/kyt.did"
  curl --fail -L -o "{{WASM_DIR}}/evm_rpc.wasm.gz"         "https://github.com/internet-computer-protocol/evm-rpc-canister/releases/download/{{IC_EVM_RPC_TAG}}/evm_rpc.wasm.gz"
  curl --fail -L -o "{{WASM_DIR}}/ic-btc-canister.wasm.gz" "https://github.com/dfinity/bitcoin-canister/releases/download/release%2F2024-07-28/ic-btc-canister.wasm.gz"


# Fetch evmc and signature-verification canister wasm and did files
[group('fetch_dependencies')]
fetch_evmc_canisters: pre_build
  just fetch_evmc_private_canister evm_testnet
  just fetch_evmc_private_canister signature_verification


# Fetch the release artifact ID from github API and download the canister wasm and did files
[private]
fetch_evmc_private_canister artifact: pre_build
  #!/usr/bin/env bash
  set -euo pipefail

  # Define variables
  owner="bitfinity-network"
  repo="evm-canister"
  GITHUB_API_TOKEN={{env("GH_API_TOKEN", "")}}
  GITHUB_BEARER_TOKEN={{env("GH_BEARER_TOKEN", "")}}
  # if the token is set, use token, else use bearer if set, otherwise exit with error
  if [ -n "$GITHUB_API_TOKEN" ]; then
    AUTH="Authorization: token $GITHUB_API_TOKEN"
    echo "Using API token"
  elif [ -n "$GITHUB_BEARER_TOKEN" ]; then
    AUTH="Authorization: Bearer $GITHUB_BEARER_TOKEN"
    echo "Using bearer token"
  else
    echo "Error: No GitHub token provided; set any of GH_API_TOKEN or GH_BEARER_TOKEN"
    exit 1
  fi

  GH_REPO="https://api.github.com/repos/$owner/$repo"
  GH_LATEST="$GH_REPO/releases/tags/{{EVMC_TAG}}"
  filename="{{artifact}}.tar.gz"
  output="{{WASM_DIR}}/{{artifact}}.tar.gz"

  # Read asset id. This is required to download the asset file from a private repository
  response=$(curl -sH "$AUTH" $GH_LATEST)
  echo "$response"
  id=`echo "$response" | jq --arg name "$filename" '.assets[] | select(.name == $name).id' `
  GH_ASSET="$GH_REPO/releases/assets/$id"

  # Downloading asset file
  echo "---------------------------------------------------------------------"
  echo "Downloading"
  echo "Artifact:   {{artifact}}"
  echo "Assets Id:  $id"
  echo "Assets URL: $GH_ASSET"
  echo "filename:   $filename"
  echo "---------------------------------------------------------------------"
  curl --fail -L -o "$output" -H "$AUTH" -H 'Accept: application/octet-stream' "$GH_ASSET"

  # Untar asset file
  tar -xvf $output -C {{WASM_DIR}}
  rm -f $output
