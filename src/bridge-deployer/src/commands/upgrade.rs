use std::path::PathBuf;

use candid::Principal;
use clap::Parser;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_utils::interfaces::ManagementCanister;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use tracing::info;

/// The upgrade command.
///
/// This command is used to upgrade a canister on the IC network.
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
        identity: GenericIdentity,
        ic_host: &str,
    ) -> anyhow::Result<()> {
        info!("Upgrading canister with ID: {}", self.canister_id.to_text());

        let canister_wasm = std::fs::read(&self.wasm)?;

        let agent = ic_agent::Agent::builder()
            .with_url(ic_host)
            .with_identity(identity)
            .build()?;

        super::fetch_root_key(ic_host, &agent).await?;

        let management_canister = ManagementCanister::create(&agent);

        management_canister
            .install(&self.canister_id, &canister_wasm)
            .with_mode(InstallMode::Upgrade(None))
            .call_and_wait()
            .await?;

        info!("Canister upgraded successfully");
        println!(
            "Canister {canister_id} upgraded successfully",
            canister_id = self.canister_id
        );

        Ok(())
    }
}
