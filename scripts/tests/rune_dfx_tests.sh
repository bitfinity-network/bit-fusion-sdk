set -e

# Set up environment variables for ord operations
export ORD_BITCOIN_RPC_USERNAME=ic-btc-integration
export ORD_BITCOIN_RPC_PASSWORD="QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E="

cargo build --tests -p integration-tests --features dfx_tests

rm -f dfx_tests.log

set +e
dfx start --background --clean --enable-bitcoin 2> dfx_tests.log

sleep 10

cargo test -p integration-tests --features dfx_tests $1 -- --test-threads=1

dfx stop