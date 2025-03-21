use std::sync::Arc;

use candid::Principal;
use evm_canister_client::EvmCanisterClient;
use ic_canister_client::PocketIcClient;
use ic_exports::pocket_ic::PocketIc;

use super::init::evm_canister_init_data;
use super::BitfinityEvm;
use crate::context::CanisterType;
use crate::utils::EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS;

impl BitfinityEvm<PocketIcClient> {
    /// Create a new [`BitfinityEvm`] instance for testing.
    pub async fn pocket_ic(pocket_ic: &Arc<PocketIc>) -> Self {
        let signature = create_canister(pocket_ic).await;
        let evm = create_canister(pocket_ic).await;

        install_signature(pocket_ic, signature, evm).await;
        install_evm(pocket_ic, evm, signature).await;

        Self {
            evm,
            signature,
            evm_client: Arc::new(EvmCanisterClient::new(PocketIcClient::from_client(
                pocket_ic.clone(),
                evm,
                crate::pocket_ic_integration_test::PocketIcTestContext::admin(),
            ))),
        }
    }
}

async fn create_canister(pocket_ic: &Arc<PocketIc>) -> Principal {
    let principal = pocket_ic
        .create_canister_with_settings(
            Some(crate::pocket_ic_integration_test::PocketIcTestContext::admin()),
            None,
        )
        .await;
    pocket_ic.add_cycles(principal, u128::MAX).await;

    principal
}

async fn install_signature(pocket_ic: &Arc<PocketIc>, signature: Principal, evm: Principal) {
    println!("Installing default Signature canister with Principal {signature}",);

    let args = candid::encode_args((vec![evm],)).expect("Failed to encode arguments");
    pocket_ic
        .install_canister(
            signature,
            CanisterType::Signature.default_canister_wasm().await,
            args,
            Some(crate::pocket_ic_integration_test::PocketIcTestContext::admin()),
        )
        .await;
}

async fn install_evm(pocket_ic: &Arc<PocketIc>, evm: Principal, signature: Principal) {
    println!("Installing EVM Canister with Principal: {evm}",);

    let init_data = evm_canister_init_data(
        signature,
        crate::pocket_ic_integration_test::PocketIcTestContext::admin(),
        Some(EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS),
    );
    let args = candid::encode_args((init_data,)).expect("Failed to encode arguments");

    pocket_ic
        .install_canister(
            evm,
            CanisterType::Evm.default_canister_wasm().await,
            args,
            Some(crate::pocket_ic_integration_test::PocketIcTestContext::admin()),
        )
        .await;
}
