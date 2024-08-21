use std::path::PathBuf;

use crate::canister_manager::{compute_wasm_hash, CanisterManager, DeploymentMode};

use super::Bridge;
use candid::{Encode, Principal};
use clap::Parser;
use ic_agent::Identity;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_canister_client::IcAgentClient;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use ic_utils::interfaces::ManagementCanister;
use tracing::{debug, info, trace};

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
}

impl ReinstallCommands {
    pub async fn reinstall_canister(
        &self,
        identity: PathBuf,
        url: &str,
        canister_manager: &mut CanisterManager,
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

        let arg = match &self.bridge_type {
            Bridge::Rune { config } => {
                trace!("Preparing Rune bridge configuration");
                let config = rune_bridge::state::RuneBridgeConfig::from(config.clone());
                debug!("Rune Bridge Config : {:?}", config);
                Encode!(&config)?
            }
            Bridge::Icrc { config } => {
                trace!("Preparing ICRC bridge configuration");
                let config = bridge_did::init::BridgeInitData::from(config.clone());
                debug!("ICRC Bridge Config : {:?}", config);
                Encode!(&config)?
            }
            Bridge::Erc20 { init, erc } => {
                trace!("Preparing ERC20 bridge configuration");
                let init = bridge_did::init::BridgeInitData::from(init.clone());
                let erc = erc20_bridge::state::BaseEvmSettings::from(erc.clone());
                debug!("ERC20 Bridge Config : {:?}", init);
                debug!("ERC20 Bridge Config : {:?}", erc);
                Encode!(&init, &erc)?
            }
            Bridge::Btc { config } => {
                trace!("Preparing BTC bridge configuration");
                let config = bridge_did::init::BridgeInitData::from(config.clone());
                debug!("BTC Bridge Config : {:?}", config);
                Encode!(&config)?
            }
        };
        trace!("Bridge configuration prepared");

        management_canister
            .install(&self.canister_id, &canister_wasm)
            .with_raw_arg(arg)
            .with_mode(InstallMode::Reinstall)
            .call_and_wait()
            .await?;

        info!("Canister installed successfully");

        canister_manager.add_or_update_canister(
            self.canister_id.to_string(),
            self.bridge_type.clone(),
            compute_wasm_hash(&canister_wasm),
            DeploymentMode::Reinstall,
            self.bridge_type.clone(),
        );

        Ok(())
    }
}
