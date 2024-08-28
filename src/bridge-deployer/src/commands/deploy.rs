use std::path::PathBuf;

use clap::Parser;
use ethereum_types::H256;
use ic_agent::Identity;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use ic_utils::interfaces::ManagementCanister;
use tracing::{debug, info, trace};

use super::{BFTArgs, Bridge};
use crate::contracts::EvmNetwork;

/// The deploy command.
///
/// This command is used to deploy a bridge canister to the IC network.
/// It will also deploy the BFT bridge if the `deploy_bft` flag is set to true.
#[derive(Debug, Parser)]
pub struct DeployCommands {
    /// The type of Bridge to deploy
    #[command(subcommand)]
    bridge_type: Bridge,

    /// The path to the wasm file to deploy
    #[arg(long, value_name = "WASM_PATH")]
    wasm: PathBuf,
}

impl DeployCommands {
    /// Deploys a canister with the specified configuration.
    pub async fn deploy_canister(
        &self,
        identity: PathBuf,
        url: &str,
        network: EvmNetwork,
        pk: H256,
        deploy_bft: bool,
        bft_args: BFTArgs,
    ) -> anyhow::Result<()> {
        info!("Starting canister deployment");
        let canister_wasm = std::fs::read(&self.wasm)?;
        debug!("WASM file read successfully");

        let identity = GenericIdentity::try_from(identity.as_ref())?;
        debug!(
            "Deploying with Principal : {}",
            identity.sender().expect("No sender found")
        );

        let agent = ic_agent::Agent::builder()
            .with_url(url)
            .with_identity(identity)
            .build()?;

        let management_canister = ManagementCanister::create(&agent);

        let (canister_id,) = management_canister
            .create_canister()
            .call_and_wait()
            .await?;
        info!("Canister created with ID: {:?}", canister_id);

        let arg = self.bridge_type.init_raw_arg()?;
        trace!("Bridge configuration prepared");

        management_canister
            .install(&canister_id, &canister_wasm)
            .with_mode(InstallMode::Install)
            .with_raw_arg(arg)
            .call_and_wait()
            .await?;

        info!("Canister installed successfully with ID: {:?}", canister_id);

        if deploy_bft {
            info!("Deploying BFT bridge");
            bft_args
                .deploy_bft(network, canister_id, &self.bridge_type, pk)
                .await?;

            info!("BFT bridge deployed successfully");
        }

        info!("Canister deployed successfully");

        Ok(())
    }
}
