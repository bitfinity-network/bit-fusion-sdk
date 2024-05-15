#!/bin/bash

deploy_evm_testnet() {
  set -e
  CHAIN_ID=355113
  ADMIN_PRINCIPAL=$(dfx identity get-principal)
  dfx canister create evm_testnet
  EVM=$(dfx canister id evm_testnet)
  dfx deploy signature_verification --argument "(vec { principal \"${EVM}\" })"
  SIGNATURE_VERIFICATION=$(dfx canister id signature_verification)

  dfx deploy evm_testnet --argument "(record {
      min_gas_price = 10;
      signature_verification_principal = principal \"${SIGNATURE_VERIFICATION}\";
      log_settings = opt record {
          enable_console = true;
          in_memory_records = opt 10000;
          log_filter = opt \"warn\";
      };
      owner = principal \"${ADMIN_PRINCIPAL}\";
      genesis_accounts = vec { };
      chain_id = $CHAIN_ID;
      coinbase = \"0x0000000000000000000000000000000000000000\";
  })"

  set +e

  echo "$EVM"
}

deploy_ckbtc_ledger() {
  set -e
  dfx canister create ic-ckbtc-ledger
  dfx canister create ic-ckbtc-minter
  CKBTC_MINTER=$(dfx canister id ic-ckbtc-minter)
  CKBTC_LEDGER=$(dfx canister id ic-ckbtc-ledger)
  ADMIN_WALLET=$(dfx identity get-wallet)

  dfx deploy token --argument "(variant {Init = record {
    minting_account = record { owner = principal \"$CKBTC_MINTER\" };
    transfer_fee = 10;
    token_symbol = \"ckTESTBTC\";
    token_name = \"Chain key testnet Bitcoin\";
    metadata = vec {};
    initial_balances = vec {};
    max_memo_length = opt 100;
    archive_options = record {
        num_blocks_to_archive = 1000;
        trigger_threshold = 2000;
        max_message_size_bytes = null;
        cycles_for_archive_creation = opt 1_000_000_000_000;
        node_max_memory_size_bytes = opt 3_221_225_472;
        controller_id = principal \"$ADMIN_WALLET\"
    }
  }})"

  set +e

  echo "$CKBTC_LEDGER"
}

deploy_ckbtc_kyt() {
  set -e
  CKBTC_MINTER="$(dfx canister id ic-ckbtc-minter)"
  ADMIN_PRINCIPAL=$(dfx identity get-principal)
  dfx canister create ic-ckbtc-kyt
  CKBTC_KYT=$(dfx canister id ic-ckbtc-ledger)
  dfx deploy ic-ckbtc-kyt --argument "(variant {InitArg = record {
    api_key = \"abcdef\";
    maintainers = vec { principal \"$ADMIN_PRINCIPAL\"; };
    mode = variant { AcceptAll };
    minter_id = principal \"$CKBTC_MINTER\";
  } })"

  dfx canister call ic-ckbtc-kyt set_api_key "(record { api_key = \"abc\"; })"

  set +e

  echo "$CKBTC_KYT"
}

deploy_ckbtc_minter() {
  set -e
  CKBTC_LEDGER="$(dfx canister id ic-ckbtc-ledger)"
  CKBTC_MINTER="$(dfx canister id ic-ckbtc-minter)"
  dfx deploy ic-ckbtc-minter --argument "(variant {Init = record {
    btc_network = variant { Regtest };
    ledger_id = principal \"$CKBTC_LEDGER\";
    ecdsa_key_name = \"dfx_test_key\";
    retrieve_btc_min_amount = 5_000;
    max_time_in_queue_nanos = 420_000_000_000;
    min_confirmations = opt 1;
    kyt_fee = opt 1000;
    kyt_principal = opt principal \"$CKBTC_KYT\";
    mode = variant { GeneralAvailability };
  }})"

  set +e

  echo "$CKBTC_MINTER"
}
