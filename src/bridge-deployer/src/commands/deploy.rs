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

/// The default number of cycles to deposit to the canister
const DEFAULT_CYCLES: u128 = 2_000_000_000_000;

/// The deploy command.
///
/// This command is used to deploy a bridge canister to the IC network.
/// It will also deploy the BFT bridge if the `deploy_bft` flag is set to true.
#[derive(Debug, Parser)]
pub struct DeployCommands {
    /// The type of Bridge to deploy
    ///
    /// The bridge type to deploy. This can be one of the following:
    /// - `rune`: The Rune bridge.
    /// - `icrc`: The ICRC bridge.
    /// - `erc20`: The ERC20 bridge.
    /// - `btc`: The BTC bridge.
    #[command(subcommand)]
    bridge_type: Bridge,

    /// The path to the wasm file to deploy
    #[arg(long, value_name = "WASM_PATH")]
    wasm: PathBuf,

    /// The number of cycles to deposit to the canister
    ///
    /// If not specified, the default value is 2_000_000_000_000 (2T) cycles.
    #[arg(long, default_value_t = DEFAULT_CYCLES)]
    cycles: u128,

    /// Wallet canister ID that is used in the creation of
    /// canisters
    #[arg(long, value_name = "WALLET_CANISTER", env)]
    wallet_canister: Principal,

    /// These are extra arguments for the BFT bridge.
    #[command(flatten, next_help_heading = "BFT Bridge deployment")]
    bft_args: BFTArgs,
}

impl DeployCommands {
    /// Deploys a canister with the specified configuration.
    pub async fn deploy_canister(
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

        let deployer =
            BridgeDeployer::create(agent.clone(), self.wallet_canister, self.cycles).await?;
        deployer
            .install_wasm(&self.wasm, &self.bridge_type, InstallMode::Install, network)
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

        info!("BFT bridge deployed successfully with {bft_address}");
        println!("BFT bridge deployed with address {bft_address}");

        deployer.configure_minter(bft_address).await?;

        info!("Canister deployed successfully");

        println!(
            "Canister {canister_type} deployed with ID {canister_id}",
            canister_type = self.bridge_type.kind(),
            canister_id = deployer.bridge_principal(),
        );

        Ok(())
    }
}
