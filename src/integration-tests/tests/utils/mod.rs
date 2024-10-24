use std::path::{Path, PathBuf};
use std::time::Duration;

use candid::Principal;
use did::init::EvmCanisterInitData;
use ic_exports::ic_kit::mock_principals::bob;

#[cfg(feature = "dfx_tests")]
pub mod brc20_helper;
pub mod btc;
#[cfg(feature = "dfx_tests")]
pub mod btc_rpc_client;
pub mod error;
#[cfg(feature = "dfx_tests")]
pub mod hiro_ordinals_client;
#[cfg(feature = "dfx_tests")]
pub mod miner;
#[cfg(feature = "dfx_tests")]
pub mod ord_client;
#[cfg(feature = "dfx_tests")]
pub mod rune_helper;
pub mod token_amount;
pub mod wasm;

/// Returns the Path to the workspace root dir
pub fn get_workspace_root_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub const EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS: Duration = Duration::from_millis(500);
pub const CHAIN_ID: u64 = 355113;

/// Re-usable function to prepare the evm canister
pub fn new_evm_init_data(
    signature_verification_principal: Principal,
    principal: Option<Principal>,
) -> EvmCanisterInitData {
    #[allow(deprecated)]
    EvmCanisterInitData {
        signature_verification_principal,
        min_gas_price: 10_u64.into(),
        chain_id: CHAIN_ID,
        log_settings: Some(ic_log::LogSettings {
            enable_console: true,
            in_memory_records: None,
            log_filter: Some("info".to_string()),
        }),
        transaction_processing_interval: Some(EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS),
        owner: principal.unwrap_or(bob()),
        ..Default::default()
    }
}
