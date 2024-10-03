[group('deploy')]
deploy_signature:
    dfx deploy signature_verification --argument '(vec {})'

[group('deploy')]
deploy_evm_canister:
    dfx deploy evm_testnet --argument \
        "(record { \
            owner = principal \"$(dfx identity get-principal)\"; \
            min_gas_price = 10; \
            chain_id = 123; \
            signature_verification_principal = principal \"$(dfx canister id signature_verification)\"; \
            genesis_accounts = vec {}; \
            coinbase = \"0x0000000000000000000000000000000000000000\"; \
            log_settings = opt record { log_filter = opt \"info\"; in_memory_records = opt 10000; enable_console = true } \
        })"

[group('deploy')]
configure_signature:
    dfx canister call signature_verification add_access "(principal \"$(dfx canister id evm_testnet)\")"

[group('deploy')]
deploy_evm:
    just deploy_signature
    just deploy_evm_canister
    just configure_signature