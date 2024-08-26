use std::path::PathBuf;

use candid::{CandidType, Encode, Principal};
use clap::Parser;
use ethereum_types::H256;
use ic_agent::Identity;
use ic_canister_client::agent::identity::GenericIdentity;

use ic_utils::interfaces::management_canister::builders::InstallMode;
use ic_utils::interfaces::ManagementCanister;
use tracing::{debug, info, trace};

use crate::canister_manager::{compute_wasm_hash, CanisterManager, DeploymentMode};
use crate::contracts::{ContractDeployer, EvmNetwork};

use super::Bridge;

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
    ///
    /// # Arguments
    /// - `identity`: The path to the identity file used to authenticate with the IC.
    /// - `url`: The URL of the IC endpoint to deploy the canister to.
    ///
    /// # Returns
    /// An `anyhow::Result` indicating whether the deployment was successful.
    pub async fn deploy_canister(
        &self,
        identity: PathBuf,
        url: &str,
        canister_manager: &mut CanisterManager,
        deploy_bft: bool,
        network: EvmNetwork,
        pk: H256,
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
                Encode!(&init, &erc)?
            }
            Bridge::Btc { config } => {
                trace!("Preparing BTC bridge configuration");
                let config = bridge_did::init::BridgeInitData::from(config.clone());
                Encode!(&config)?
            }
        };
        trace!("Bridge configuration prepared");

        management_canister
            .install(&canister_id, &canister_wasm)
            .with_mode(InstallMode::Install)
            .with_raw_arg(arg)
            .call_and_wait()
            .await?;

        info!("Canister installed successfully");

        canister_manager.add_or_update_canister(
            canister_id.to_string(),
            self.bridge_type.clone(),
            compute_wasm_hash(&canister_wasm),
            DeploymentMode::Install,
            self.bridge_type.clone(),
        );

        if deploy_bft {
            let contract_deployer = ContractDeployer::new(network, pk);

            let bft_address = contract_deployer
                .de
        }

        Ok(())
    }
}
