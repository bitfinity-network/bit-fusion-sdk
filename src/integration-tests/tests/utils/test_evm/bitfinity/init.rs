use std::time::Duration;

use candid::Principal;
use did::init::EvmCanisterInitData;

use crate::utils::CHAIN_ID;

pub fn evm_canister_init_data(
    signature_verification_principal: Principal,
    owner: Principal,
    transaction_processing_interval: Option<Duration>,
) -> EvmCanisterInitData {
    #[allow(deprecated)]
    EvmCanisterInitData {
        signature_verification_principal,
        min_gas_price: 10_u64.into(),
        chain_id: CHAIN_ID,
        log_settings: Some(ic_log::LogSettings {
            enable_console: true,
            in_memory_records: None,
            log_filter: Some("debug".to_string()),
        }),
        transaction_processing_interval,
        owner,
        ..Default::default()
    }
}
