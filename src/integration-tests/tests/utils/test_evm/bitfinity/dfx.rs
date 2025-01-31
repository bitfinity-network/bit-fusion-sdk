use std::sync::Arc;
use std::time::Duration;

use candid::Principal;
use evm_canister_client::EvmCanisterClient;
use ic_canister_client::IcAgentClient;
use ic_exports::ic_kit::mock_principals::bob;
use ic_test_utils::{get_agent, Agent, Canister};
use ic_utils::interfaces::ManagementCanister;

use super::init::evm_canister_init_data;
use super::BitfinityEvm;
use crate::context::CanisterType;
use crate::utils::EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS;

const DFX_URL: &str = "http://127.0.0.1:4943";
const ADMIN: &str = "max";
const INIT_CANISTER_CYCLES: u64 = 90_000_000_000_000;

impl BitfinityEvm<IcAgentClient> {
    /// Create a new [`BitfinityEvm`] instance for testing.
    pub async fn dfx() -> Self {
        let url = Some(DFX_URL);
        let max = get_agent(ADMIN, url, Some(Duration::from_secs(180)))
            .await
            .expect("Failed to get agent");

        let signature = create_canister(&max).await;
        let evm = create_canister(&max).await;

        install_signature(&max, signature, evm).await;
        install_evm(&max, evm, signature).await;

        Self {
            evm,
            signature,
            evm_client: Arc::new(EvmCanisterClient::new(IcAgentClient::with_agent(evm, max))),
        }
    }
}

async fn create_canister(agent: &Agent) -> Principal {
    let wallet = Canister::new_wallet(agent, ADMIN).unwrap();
    wallet
        .create_canister(INIT_CANISTER_CYCLES, None)
        .await
        .expect("Failed to create canister")
}

async fn install_signature(agent: &Agent, signature: Principal, evm: Principal) {
    println!("Installing default Signature canister with Principal {signature}",);

    let mng = ManagementCanister::create(agent);
    mng.install(
        &signature,
        &CanisterType::Signature.default_canister_wasm().await,
    )
    .with_args((vec![evm],))
    .call_and_wait()
    .await
    .expect("Failed to install signature canister");
}

async fn install_evm(agent: &Agent, evm: Principal, signature: Principal) {
    println!("Installing EVM Canister with Principal: {evm}",);

    let init_data = evm_canister_init_data(
        signature,
        bob(),
        Some(EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS),
    );
    let mng = ManagementCanister::create(agent);
    mng.install(&evm, &CanisterType::Evm.default_canister_wasm().await)
        .with_args((init_data,))
        .call_and_wait()
        .await
        .expect("Failed to install evm canister");
}
