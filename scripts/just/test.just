
# Run all unit tests and integration tests
[group('test')]
all_test: test integration_test

# Run all unit tests
[group('test')]
test:
  cargo test

# Run all integration tests
[group('test')]
integration_test: 
  export PROTOC_INCLUDE=${PWD}/proto
  cargo test -p integration-tests --no-default-features --features "pocket_ic_integration_test" --features "state_machine_tests"

# Run all unit tests for the i686 target
[group('test')]
test_i686:
  cargo test --target i686-unknown-linux-gnu

# Run all dfx tests 
[group('test')]
dfx_test: 
  echo "Running tests on dfx... TODO"