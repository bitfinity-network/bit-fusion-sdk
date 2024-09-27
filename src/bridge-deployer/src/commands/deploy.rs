use std::path::PathBuf;

use candid::Principal;
use clap::Parser;
use ethereum_types::H256;
use ic_agent::Identity;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use ic_utils::interfaces::{ManagementCanister, WalletCanister};
use tracing::{debug, info, trace};

use super::{BFTArgs, Bridge};
use crate::canister_ids::{CanisterIds, CanisterIdsPath};
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
    #[command(flatten, next_help_heading = "Bridge Contract Args")]
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
        deploy_bft: bool,
        canister_ids_path: CanisterIdsPath,
    ) -> anyhow::Result<()> {
        info!("Starting canister deployment");
        let mut canister_ids = CanisterIds::read_or_default(canister_ids_path);
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

        info!("Using  wallet canister ID: {}", self.wallet_canister);
        let wallet = WalletCanister::create(&agent, self.wallet_canister).await?;

        let canister_id = wallet
            .wallet_create_canister(self.cycles, None, None, None, None)
            .await?
            .canister_id;

        // save to canister ids
        canister_ids.set((&self.bridge_type).into(), canister_id);

        let management_canister = ManagementCanister::create(&agent);

        let arg = self.bridge_type.init_raw_arg()?;
        trace!("Bridge configuration prepared");

        management_canister
            .install(&canister_id, &canister_wasm)
            .with_mode(InstallMode::Install)
            .with_raw_arg(arg)
            .call_and_wait()
            .await?;

        info!("Canister installed successfully with ID: {}", canister_id);

        if deploy_bft {
            info!("Deploying BFT bridge");
            self.bft_args
                .deploy_bft(network, canister_id, &self.bridge_type, pk, &agent)
                .await?;

            info!("BFT bridge deployed successfully");
        }

        // write canister ids file
        canister_ids.write()?;

        info!("Canister deployed successfully");

        Ok(())
    }
}
