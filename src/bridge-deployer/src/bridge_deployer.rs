use alloy::primitives::Address;
use anyhow::anyhow;
use bridge_client::{BridgeCanisterClient, GenericBridgeClient};
use bridge_did::evm_link::EvmLink;
use candid::Principal;
use ic_agent::Agent;
use ic_canister_client::IcAgentClient;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use ic_utils::interfaces::{ManagementCanister, WalletCanister};
use tracing::{debug, info};

use crate::commands::Bridge;
use crate::contracts::IcNetwork;

pub struct BridgeDeployer {
    client: GenericBridgeClient<IcAgentClient>,
    agent: Agent,
}

impl BridgeDeployer {
    pub async fn create(agent: Agent, wallet: Principal, cycles: u128) -> anyhow::Result<Self> {
        info!("Using wallet canister ID: {wallet}");
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
        canister_wasm: &[u8],
        config: &Bridge,
        mode: InstallMode,
        network: IcNetwork,
        evm: EvmLink,
    ) -> anyhow::Result<Principal> {
        debug!(
            "WASM file read successfully. File size: {}",
            canister_wasm.len()
        );

        let canister_id = self.client.client().canister_id;
        let management_canister = ManagementCanister::create(&self.agent);
        let arg = config.init_raw_arg(
            self.agent.get_principal().expect("invalid agent identity"),
            network,
            evm,
        )?;

        management_canister
            .install(&canister_id, canister_wasm)
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

    pub async fn configure_minter(&self, btf_address: Address) -> anyhow::Result<()> {
        info!("Configuring bridge canister");

        self.client
            .set_btf_bridge_contract(&btf_address.into())
            .await?;

        info!("Bridge canister is configured");
        Ok(())
    }

    pub fn bridge_principal(&self) -> Principal {
        self.client.client().canister_id
    }
}
