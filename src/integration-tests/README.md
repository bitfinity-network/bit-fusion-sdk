# Building and running tests

To build integration tests, use one of the features:

```
cargo test -p integration-tests --features pocket_ic_integration_tests
cargo test -p integration-tests --features state_machine_integration_tests
```

Building state machine integration tests require some libraries to be installed:

```
# Code for Debian/Ubuntu:
sudo apt-get install liblmdb-dev libunwind-dev
```
