
# Cleans the build artifacts
[group('build')]
[confirm("Are you sure you want to clean the build artifacts?")]
clean:
  rm -rf {{WASM_DIR}}
  cargo clean


# Builds the solidity contracts
[group('build')]
build_solidity:
  cd solidity && forge soldeer install
  cd solidity && forge test -vv
  cd solidity && forge build --force


# Builds all canisters
[group('build')]
build_all_canisters: build_icrc2_bridge build_erc20_bridge build_brc20_bridge build_btc_bridge build_rune_bridge


# Builds the icrc2 bridge canister
[group('build')]
build_icrc2_bridge: pre_build
  just build_canister "icrc2_bridge" "export-api" "icrc2-bridge"
  

# Builds the erc20 bridge canister
[group('build')]
build_erc20_bridge: pre_build
  just build_canister "erc20_bridge" "export-api" "erc20-bridge"


# Builds the brc20 bridge canister
[group('build')]
build_brc20_bridge: pre_build
  just build_canister "brc20_bridge" "export-api" "brc20-bridge"


# Builds the btc bridge canister
[group('build')]
build_btc_bridge: pre_build
  just build_canister "btc_bridge" "export-api" "btc-bridge"


# Builds the rune bridge canister
[group('build')]
build_rune_bridge: pre_build
  just build_canister "rune_bridge" "export-api" "rune-bridge" 
  


[private]
pre_build: 
  mkdir -p "{{WASM_DIR}}"


# Builds a canister with the given name and features, generates did file then shrinks and gzips the wasm file.
# The output wasm file is saved in the WASM_DIR directory.
[private]
build_canister canister_name features output_wasm:
  echo "Building {{canister_name}} Canister with features: {{features}}"
  cargo build --target wasm32-unknown-unknown --release --package "{{canister_name}}" --features "{{features}}"
  ic-wasm "target/wasm32-unknown-unknown/release/{{canister_name}}.wasm" -o "{{WASM_DIR}}/{{output_wasm}}.wasm" shrink
  candid-extractor "{{WASM_DIR}}/{{output_wasm}}.wasm" > "{{WASM_DIR}}/{{output_wasm}}.did"
  gzip -k "{{WASM_DIR}}/{{output_wasm}}.wasm" --force