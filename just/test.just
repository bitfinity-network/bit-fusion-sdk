TEST_LOGFILE:="./target/integration_tests.log"

# Run all unit tests and integration tests
[group('test')]
test_all: test integration_test


# Run all unit tests
[group('test')]
test test_name="":
  cargo test {{test_name}}


# Run pocket-ic and state machine integration tests
[group('test')]
integration_test test_name="": docker_down docker_pull docker_up
  export PROTOC_INCLUDE=${PWD}/proto
  cargo test {{test_name}} -p integration-tests --no-default-features --features "pocket_ic_integration_test" --features "state_machine_tests" 2> "{{TEST_LOGFILE}}"


# Run all unit tests for the i686 target
[group('test')]
test_i686 test_name="":
  cargo test {{test_name}} --target i686-unknown-linux-gnu
