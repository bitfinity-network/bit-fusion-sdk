use std::path::PathBuf;

use candid::Principal;
use clap::Parser;
use ethereum_types::H160;
use ic_agent::Agent;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use tracing::info;

use super::Bridge;
use crate::bridge_deployer::BridgeDeployer;
use crate::canister_ids::{CanisterIds, CanisterIdsPath};
use crate::contracts::EvmNetwork;

/// The reinstall command.
///
/// This command is used to reinstall a bridge canister to the IC network.
#[derive(Debug, Parser)]
pub struct ReinstallCommands {
    /// The type of Bridge to reinstall
    /// This can be one of the following:
    /// - `rune`: The Rune bridge.
    /// - `icrc`: The ICRC bridge.
    /// - `erc20`: The ERC20 bridge.
    /// - `btc`: The BTC bridge.
    #[command(subcommand)]
    bridge_type: Bridge,

    /// The canister ID of the bridge to reinstall.
    ///
    /// If not provided, it will be fetched from the `canister_ids.json` file
    #[arg(long, value_name = "CANISTER_ID")]
    canister_id: Option<Principal>,

    /// The path to the wasm file to deploy
    #[arg(long, value_name = "WASM_PATH")]
    wasm: PathBuf,

    /// Existing BFT bridge contract address to work with the deployed bridge.
    #[arg(long = "bft-bridge", value_name = "ADDRESS")]
    bft_bridge: H160,
}

impl ReinstallCommands {
    pub async fn reinstall_canister(
        &self,
        identity: GenericIdentity,
        ic_host: &str,
        network: EvmNetwork,
        canister_ids_path: CanisterIdsPath,
    ) -> anyhow::Result<()> {
        info!("Starting canister reinstall");

        let canister_ids = CanisterIds::read_or_default(canister_ids_path);

        // get canister id
        let canister = (&self.bridge_type).into();
        let canister_id = match self.canister_id.or_else(|| canister_ids.get(canister)) {
            Some(id) => id,
            None => {
                anyhow::bail!("Could not resolve canister id for {canister}");
            }
        };

        let agent = Agent::builder()
            .with_url(ic_host)
            .with_identity(identity)
            .build()?;

        super::fetch_root_key(ic_host, &agent).await?;

        let deployer = BridgeDeployer::new(agent.clone(), canister_id);
        deployer
            .install_wasm(
                &self.wasm,
                &self.bridge_type,
                InstallMode::Reinstall,
                network,
            )
            .await?;

        info!("Canister installed successfully");

        deployer.configure_minter(self.bft_bridge).await?;

        info!("Canister reinstalled successfully with ID: {}", canister_id);

        println!(
            "Canister {canister_type} reinstalled with ID {canister_id}",
            canister_type = self.bridge_type.kind(),
        );

        Ok(())
    }
}
