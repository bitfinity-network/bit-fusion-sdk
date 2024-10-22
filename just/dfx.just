
DFX_LOGFILE:="./target/dfx_tests.log"


# Start the local dfx server
[group('dfx')]
dfx_start: dfx_prepare_env dfx_stop
  # Start the local dfx server
  dfx start --background --clean --enable-bitcoin  --artificial-delay 0 2> "{{DFX_LOGFILE}}"

  # Add cycles to the wallet
  wallet_principal=$(dfx identity get-wallet) && dfx ledger fabricate-cycles --t 1000000 --canister $wallet_principal
  sleep 2


# Start the ICX Proxy. Requires a running dfx server
[group('dfx')]
dfx_start_proxy:
  killall -9 icx-proxy || true
  dfx_local_port=$(dfx info replica-port) && icx-proxy --fetch-root-key --address 0.0.0.0:8545 --dns-alias 0.0.0.0:bd3sg-teaaa-aaaaa-qaaba-cai --replica http://localhost:$dfx_local_port &
  sleep 2


# Stop the local dfx server and the ICX Proxy
[group('dfx')]
dfx_stop:
  killall -9 icx-proxy || true
  dfx stop


# Start docker, dfx and run the dfx tests
[group('dfx')]
dfx_test test_name="": docker_down docker_pull docker_up dfx_start dfx_start_proxy
  #!/usr/bin/env bash
  sleep 5
  ( 
    set +e
    cargo test {{test_name}} -p integration-tests --features dfx_tests
  )
  TEST_RESULT=$?
  just dfx_stop docker_down
  exit $TEST_RESULT


[private]
dfx_prepare_env: 
  dfx identity new --storage-mode=plaintext --force max
  dfx identity new --storage-mode=plaintext --force alice
  dfx identity new --storage-mode=plaintext --force alex
  dfx identity use max
