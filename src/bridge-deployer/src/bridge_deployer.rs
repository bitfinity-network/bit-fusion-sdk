use std::path::PathBuf;

use anyhow::anyhow;
use bridge_client::{BridgeCanisterClient, GenericBridgeClient};
use candid::Principal;
use ethereum_types::H160;
use ic_agent::Agent;
use ic_canister_client::IcAgentClient;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use ic_utils::interfaces::{ManagementCanister, WalletCanister};
use tracing::{debug, info};

use crate::commands::Bridge;
use crate::contracts::EvmNetwork;

pub struct BridgeDeployer {
    client: GenericBridgeClient<IcAgentClient>,
    agent: Agent,
}

impl BridgeDeployer {
    pub async fn create(agent: Agent, wallet: Principal, cycles: u128) -> anyhow::Result<Self> {
        info!("Using  wallet canister ID: {wallet}");
        let wallet = WalletCanister::create(&agent, wallet).await?;
        let caller = agent.get_principal().map_err(|err| anyhow!(err))?;

        let canister_id = wallet
            .wallet_create_canister(cycles, Some(vec![caller]), None, None, None)
            .await?
            .canister_id;

        let client =
            GenericBridgeClient::new(IcAgentClient::with_agent(canister_id, agent.clone()));
        Ok(Self { client, agent })
    }

    pub fn new(agent: Agent, bridge_principal: Principal) -> Self {
        let client =
            GenericBridgeClient::new(IcAgentClient::with_agent(bridge_principal, agent.clone()));
        Self { client, agent }
    }

    pub async fn install_wasm(
        &self,
        wasm_path: &PathBuf,
        config: &Bridge,
        mode: InstallMode,
        network: EvmNetwork,
        evm: Principal,
    ) -> anyhow::Result<Principal> {
        let canister_wasm = std::fs::read(wasm_path)?;
        debug!(
            "WASM file read successfully. File size: {}",
            canister_wasm.len()
        );

        let canister_id = self.client.client().canister_id;
        let management_canister = ManagementCanister::create(&self.agent);
        let arg = config.init_raw_arg(network, evm)?;

        management_canister
            .install(&canister_id, &canister_wasm)
            .with_mode(mode)
            .with_raw_arg(arg)
            .call_and_wait()
            .await?;

        info!(
            "Canister code installed successfully with ID: {}",
            canister_id
        );

        Ok(canister_id)
    }

    pub async fn configure_minter(&self, bft_address: H160) -> anyhow::Result<()> {
        info!("Configuring bridge canister");

        self.client
            .set_bft_bridge_contract(&bft_address.into())
            .await?;

        info!("Bridge canister is configured");
        Ok(())
    }

    pub fn bridge_principal(&self) -> Principal {
        self.client.client().canister_id
    }
}
