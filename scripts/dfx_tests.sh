set -e

cargo build --tests -p integration-tests --features dfx_tests

rm -f dfx_tests.log

set +e
dfx start --background --clean --enable-bitcoin 2> dfx_tests.log

sleep 10

cargo test -p integration-tests --features dfx_tests

dfx stop