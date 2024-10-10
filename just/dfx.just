
DFX_LOGFILE:="./target/dfx_tests.log"


# Start the local dfx server
[group('dfx')]
dfx_start: dfx_prepare_env dfx_stop
  # Start the local dfx server
  dfx start --background --clean --enable-bitcoin 2> "{{DFX_LOGFILE}}"

  # Add cycles to the wallet
  wallet_principal=$(dfx identity get-wallet) && dfx ledger fabricate-cycles --t 1000000 --canister $wallet_principal
  sleep 2


# Stop the local dfx server and the ICX Proxy
[group('dfx')]
dfx_stop:
  dfx stop


# Start docker, dfx and run the dfx tests
[group('dfx')]
dfx_test test_name="": docker_down docker_pull docker_up dfx_start
  #!/usr/bin/env bash
  sleep 5

  cleanup() {
    just dfx_stop docker_down
  }  
  trap cleanup EXIT INT TERM

  cargo test {{test_name}} -p integration-tests --features dfx_tests



[private]
dfx_prepare_env: 
  dfx identity new --storage-mode=plaintext --force max
  dfx identity new --storage-mode=plaintext --force alice
  dfx identity new --storage-mode=plaintext --force alex
  dfx identity use max
