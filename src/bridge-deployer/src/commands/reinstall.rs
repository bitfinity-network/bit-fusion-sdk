use std::path::PathBuf;

use bridge_did::evm_link::EvmLink;
use candid::Principal;
use clap::Parser;
use ethereum_types::H160;
use ic_agent::Agent;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use tracing::info;

use super::Bridge;
use crate::bridge_deployer::BridgeDeployer;
use crate::canister_ids::{CanisterIds, CanisterIdsPath, CanisterType};
use crate::contracts::IcNetwork;
use crate::evm::ic_host;

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

    /// The path to the wasm file to deploy. If not set, the default wasm file will be used.
    ///
    /// Latest build of the wasm files are already embedded in the binary.
    #[arg(long, value_name = "WASM_PATH")]
    wasm: Option<PathBuf>,

    /// Existing BTF bridge contract address to work with the deployed bridge.
    #[arg(long = "btf-bridge", value_name = "ADDRESS")]
    btf_bridge: H160,
}

impl ReinstallCommands {
    pub async fn reinstall_canister(
        &self,
        identity: GenericIdentity,
        network: IcNetwork,
        canister_ids_path: CanisterIdsPath,
        evm_link: EvmLink,
    ) -> anyhow::Result<()> {
        info!("Starting canister reinstall");

        let ic_host = ic_host(network);
        let canister_ids = CanisterIds::read_or_default(canister_ids_path);

        // get canister id
        let canister: CanisterType = (&self.bridge_type).into();
        let canister_id = match self
            .canister_id
            .or_else(|| canister_ids.get(canister.clone()))
        {
            Some(id) => id,
            None => {
                anyhow::bail!("Could not resolve canister id for {canister}");
            }
        };

        let agent = Agent::builder()
            .with_url(&ic_host)
            .with_identity(identity)
            .build()?;

        super::fetch_root_key(&ic_host, &agent).await?;

        let canister_wasm_path = self
            .wasm
            .as_deref()
            .unwrap_or_else(|| super::wasm::get_default_wasm_path(&self.bridge_type));
        let canister_wasm = std::fs::read(canister_wasm_path)?;

        let deployer = BridgeDeployer::new(agent.clone(), canister_id);
        deployer
            .install_wasm(
                &canister_wasm,
                &self.bridge_type,
                InstallMode::Reinstall,
                network,
                evm_link,
            )
            .await?;

        info!("Canister installed successfully");

        deployer.configure_minter(self.btf_bridge).await?;

        info!("Canister reinstalled successfully with ID: {}", canister_id);

        println!(
            "Canister {canister_type} reinstalled with ID {canister_id}",
            canister_type = self.bridge_type.kind(),
        );

        Ok(())
    }
}
