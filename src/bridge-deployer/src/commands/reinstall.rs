use std::path::PathBuf;

use candid::Principal;
use clap::Parser;
use ethereum_types::H256;
use ic_agent::{Agent, Identity};
use ic_canister_client::agent::identity::GenericIdentity;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use tracing::{debug, info};

use super::{BFTArgs, Bridge};
use crate::bridge_deployer::BridgeDeployer;
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

    #[arg(long, value_name = "CANISTER_ID")]
    canister_id: Principal,

    /// The path to the wasm file to deploy
    #[arg(long, value_name = "WASM_PATH")]
    wasm: PathBuf,

    /// These are extra arguments for the BFT bridge.
    #[command(flatten, next_help_heading = "Bridge Contract Args")]
    bft_args: BFTArgs,
}

impl ReinstallCommands {
    pub async fn reinstall_canister(
        &self,
        identity: PathBuf,
        ic_host: &str,
        network: EvmNetwork,
        pk: H256,
    ) -> anyhow::Result<()> {
        let identity = GenericIdentity::try_from(identity.as_ref())?;
        let caller = identity.sender().expect("No sender found");
        debug!("Deploying with Principal : {caller}",);

        let agent = Agent::builder()
            .with_url(ic_host)
            .with_identity(identity)
            .build()?;

        super::fetch_root_key(ic_host, &agent).await?;

        let deployer = BridgeDeployer::new(agent.clone(), self.canister_id);
        deployer
            .install_wasm(&self.wasm, &self.bridge_type, InstallMode::Reinstall)
            .await?;
        let bft_address = self
            .bft_args
            .deploy_bft(
                network,
                deployer.bridge_principal(),
                &self.bridge_type,
                pk,
                &agent,
            )
            .await?;

        deployer.configure_minter(bft_address).await?;

        info!("Canister deployed successfully");

        Ok(())
    }
}
