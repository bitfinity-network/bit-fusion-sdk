use std::path::PathBuf;

use candid::Principal;
use clap::Parser;
use ethereum_types::H256;
use ic_agent::Identity;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use ic_utils::interfaces::ManagementCanister;
use tracing::{debug, info, trace};

use super::{BFTArgs, Bridge};
use crate::contracts::EvmNetwork;

#[derive(Debug, Parser)]
pub struct ReinstallCommands {
    /// The type of Bridge to deploy
    #[command(subcommand)]
    bridge_type: Bridge,

    #[arg(long, value_name = "CANISTER_ID")]
    canister_id: Principal,

    /// The path to the wasm file to deploy
    #[arg(long, value_name = "WASM_PATH")]
    wasm: PathBuf,

    #[command(flatten)]
    args: BFTArgs,
}

impl ReinstallCommands {
    pub async fn reinstall_canister(
        &self,
        identity: PathBuf,
        url: &str,
        network: EvmNetwork,
        pk: H256,
        deploy_bft: bool,
        bft_args: BFTArgs,
    ) -> anyhow::Result<()> {
        info!("Starting canister reinstall");
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

        let arg = self.bridge_type.init_raw_arg()?;
        trace!("Bridge configuration prepared");

        management_canister
            .install(&self.canister_id, &canister_wasm)
            .with_raw_arg(arg)
            .with_mode(InstallMode::Reinstall)
            .call_and_wait()
            .await?;

        info!("Canister installed successfully");

        if deploy_bft {
            info!("Deploying BFT bridge");
            bft_args
                .deploy_bft(network, self.canister_id, &self.bridge_type, pk)
                .await?;

            info!("BFT bridge deployed successfully");
        }

        info!(
            "Canister reinstalled successfully with ID: {:?}",
            self.canister_id
        );
        Ok(())
    }
}
