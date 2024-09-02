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
        deploy_bft: bool,
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
            .with_url(ic_host)
            .with_identity(identity)
            .build()?;

        super::fetch_root_key(ic_host, &agent).await?;

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
            self.bft_args
                .deploy_bft(network, self.canister_id, &self.bridge_type, pk, &agent)
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
