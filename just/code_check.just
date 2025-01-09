# Formats the code
[group('code_check')]
fmt args="":
  cargo fmt --all {{args}}


# Formats the code using the nightly version of rustfmt
[group('code_check')]
fmt_nightly:
  cargo +nightly fmt --all


# Runs clippy on the code
[group('code_check')]
clippy args="": init_dummy_artifacts
  export PROTOC_INCLUDE=${PWD}/proto
  cargo clippy --all-features --all-targets {{args}}


# Runs fmt and clippy on the code. It fails if any of the checks fail
[group('code_check')]
check_code:
  just fmt "--check"
  just clippy "-- -D warnings"

# Init dummy artifacts because are required for building bridge-deplopyer
[private]
init_dummy_artifacts:
  mkdir -p .artifact/
  touch .artifact/brc20-bridge.wasm.gz
  touch .artifact/btc-bridge.wasm.gz
  touch .artifact/erc20-bridge.wasm.gz
  touch .artifact/icrc2-bridge.wasm.gz
  touch .artifact/rune-bridge.wasm.gz
