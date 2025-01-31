use std::sync::Arc;

use candid::Principal;
use evm_canister_client::EvmCanisterClient;
use ic_canister_client::PocketIcClient;
use ic_exports::ic_kit::mock_principals::bob;
use ic_exports::pocket_ic::PocketIc;

use super::init::evm_canister_init_data;
use super::BitfinityEvm;
use crate::context::CanisterType;
use crate::utils::EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS;

impl BitfinityEvm<PocketIcClient> {
    /// Create a new [`BitfinityEvm`] instance for testing.
    pub async fn pocket_ic(pocket_ic: &Arc<PocketIc>) -> Self {
        let signature = pocket_ic.create_canister().await;
        let evm = pocket_ic.create_canister().await;

        install_signature(pocket_ic, signature, evm).await;
        install_evm(pocket_ic, evm, signature).await;

        Self {
            evm,
            signature,
            evm_client: Arc::new(EvmCanisterClient::new(PocketIcClient::from_client(
                pocket_ic.clone(),
                evm,
                bob(),
            ))),
        }
    }
}

async fn install_signature(pocket_ic: &Arc<PocketIc>, signature: Principal, evm: Principal) {
    println!("Installing default Signature canister with Principal {signature}",);

    let args = candid::encode_args((vec![evm],)).expect("Failed to encode arguments");
    pocket_ic
        .install_canister(
            signature,
            CanisterType::Signature.default_canister_wasm().await,
            args,
            Some(bob()),
        )
        .await;
}

async fn install_evm(pocket_ic: &Arc<PocketIc>, evm: Principal, signature: Principal) {
    println!("Installing EVM Canister with Principal: {evm}",);

    let init_data = evm_canister_init_data(
        signature,
        bob(),
        Some(EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS),
    );
    let args = candid::encode_args((init_data,)).expect("Failed to encode arguments");

    pocket_ic
        .install_canister(
            evm,
            CanisterType::Evm.default_canister_wasm().await,
            args,
            Some(bob()),
        )
        .await;
}
