use std::path::PathBuf;

use candid::{CandidType, Encode, Principal};
use clap::Parser;
use ic_agent::agent::http_transport::reqwest_transport::reqwest::Identity;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_canister_client::IcAgentClient;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use ic_utils::interfaces::ManagementCanister;
use tracing::info;

use crate::canister_manager::{compute_wasm_hash, CanisterManager, DeploymentMode};

#[derive(Debug, Parser)]
pub struct UpgradeCommands {
    #[arg(long, value_name = "CANISTER_ID")]
    canister_id: Principal,

    /// The path to the wasm file to deploy
    #[arg(long, value_name = "WASM_PATH")]
    wasm: PathBuf,
}

impl UpgradeCommands {
    pub async fn upgrade_canister(
        &self,
        identity: PathBuf,
        url: &str,
        canister_manager: &mut CanisterManager,
    ) -> anyhow::Result<()> {
        info!("Upgrading canister with ID: {}", self.canister_id.to_text());

        let canister_wasm = std::fs::read(&self.wasm)?;

        let identity = GenericIdentity::try_from(identity.as_ref())?;

        let agent = ic_agent::Agent::builder()
            .with_url(url)
            .with_identity(identity)
            .build()?;

        let management_canister = ManagementCanister::create(&agent);

        management_canister
            .install(&self.canister_id, &canister_wasm)
            .with_mode(InstallMode::Upgrade {
                skip_pre_upgrade: None,
            })
            .call_and_wait()
            .await?;

        info!("Canister upgraded successfully");

        let info = canister_manager
            .get_canister(&self.canister_id.to_text())
            .expect("Canister not found");

        canister_manager.add_or_update_canister(
            self.canister_id.to_string(),
            info.canister_type.clone(),
            compute_wasm_hash(&canister_wasm),
            DeploymentMode::Upgrade,
            info.canister_type.clone(),
        );

        Ok(())
    }
}
