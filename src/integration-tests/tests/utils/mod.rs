use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use candid::Principal;
use did::init::EvmCanisterInitData;
use ethers_core::abi::{Function, Param, StateMutability};
use ic_exports::ic_kit::mock_principals::bob;
use ic_log::LogSettings;
use once_cell::sync::OnceCell;
use solidity_helper::{SolidityBuilder, SolidityContract};

pub mod btc;
pub mod error;

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
    EvmCanisterInitData {
        signature_verification_principal,
        min_gas_price: 10_u64.into(),
        chain_id: CHAIN_ID,
        log_settings: Some(LogSettings {
            enable_console: true,
            in_memory_records: None,
            log_filter: Some("info".to_string()),
        }),
        transaction_processing_interval: Some(EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS),
        owner: principal.unwrap_or(bob()),
        ..Default::default()
    }
}

pub fn get_solidity_smart_contracts() -> &'static HashMap<String, SolidityContract> {
    static INSTANCE: OnceCell<HashMap<String, SolidityContract>> = OnceCell::new();
    INSTANCE.get_or_init(|| {
        let builder = SolidityBuilder::new().unwrap();
        builder
            .build_updated_contracts()
            .expect("Should compile solidity smart contracts")
            .contracts
    })
}

/// Returns the function selector for the given function name and parameters.
#[allow(deprecated)]
pub fn function_selector(name: &str, params: &[Param]) -> Function {
    Function {
        name: name.to_owned(),
        inputs: params.to_vec(),
        outputs: vec![],
        constant: None,
        state_mutability: StateMutability::NonPayable,
    }
}
