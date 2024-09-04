use std::path::PathBuf;
use std::time::Duration;

use bridge_did::error::BftResult;
use candid::{Encode, Principal};
use clap::{Parser, Subcommand};
use deploy::DeployCommands;
use ethereum_types::{H160, H256};
use ic_agent::Agent;
use ic_canister_client::{CanisterClient, IcAgentClient};
use reinstall::ReinstallCommands;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};
use upgrade::UpgradeCommands;

use crate::config;
use crate::contracts::{EvmNetwork, SolidityContractDeployer};

mod deploy;
mod reinstall;
mod upgrade;

/// The commands that can be run by the bridge deployer.
#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(
        name = "deploy",
        about = "Deploy a Bridge",
        next_help_heading = "Deploy Bridge"
    )]
    Deploy(DeployCommands),

    #[command(
        name = "reinstall",
        about = "Reinstall a Bridge",
        next_help_heading = "Reinstall Bridge"
    )]
    Reinstall(ReinstallCommands),

    #[command(
        name = "upgrade",
        about = "Upgrade a Bridge",
        next_help_heading = "Upgrade Bridge"
    )]
    Upgrade(UpgradeCommands),
}

#[derive(Subcommand, Clone, Serialize, Deserialize, Debug)]
pub enum Bridge {
    Brc20 {
        /// The configuration to use
        #[command(flatten)]
        config: config::InitBridgeConfig,
        /// Extra configuration for the BRC20 bridge
        #[command(name = "brc20", flatten)]
        brc20: config::Brc20BridgeConfig,
    },
    Btc {
        /// The configuration to use
        #[command(flatten)]
        config: config::InitBridgeConfig,
    },
    Erc20 {
        /// The configuration to use
        #[command(flatten)]
        init: config::InitBridgeConfig,
        /// Extra configuration for the ERC20 bridge
        #[command(name = "erc", flatten)]
        erc: config::BaseEvmSettingsConfig,
    },
    Icrc {
        /// The configuration to use
        #[command(flatten)]
        config: config::InitBridgeConfig,
    },
    Rune {
        /// Bridge configuration
        #[command(flatten)]
        init: config::InitBridgeConfig,
        /// Rune bridge configuration
        #[command(flatten, name = "rune")]
        rune: config::RuneBridgeConfig,
    },
}

impl Bridge {
    /// Initialize the raw argument for the bridge
    pub fn init_raw_arg(&self) -> anyhow::Result<Vec<u8>> {
        let arg = match &self {
            Bridge::Brc20 {
                config: init,
                brc20,
            } => {
                trace!("Preparing BRC20 bridge configuration");
                let init_data = bridge_did::init::BridgeInitData::from(init.clone());
                debug!("BRC20 Bridge Config : {:?}", init_data);
                let brc20_config = brc20_bridge::state::Brc20BridgeConfig::from(brc20.clone());
                Encode!(&init_data, &brc20_config)?
            }
            Bridge::Rune { init, rune } => {
                trace!("Preparing Rune bridge configuration");
                let init_data = bridge_did::init::BridgeInitData::from(init.clone());
                debug!("Init Bridge Config : {:?}", init_data);
                let rune_config = rune_bridge::state::RuneBridgeConfig::from(rune.clone());
                debug!("Rune Bridge Config : {:?}", rune_config);
                Encode!(&init_data, &rune_config)?
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

        Ok(arg)
    }

    /// Returns if the bridge is wrapped side or not
    pub fn is_wrapped_side(&self) -> bool {
        match self {
            Bridge::Brc20 { .. } => true,
            Bridge::Rune { .. } => true,
            Bridge::Icrc { .. } => true,
            Bridge::Erc20 { .. } => false,
            Bridge::Btc { .. } => true,
        }
    }
}

impl Commands {
    /// Runs the specified command for the bridge deployer.
    ///
    /// This function handles the deployment, reinstallation, and upgrade of the bridge canister.
    /// It takes in various parameters such as the identity file path, the IC host, the Ethereum network,
    /// the private key, whether to deploy the BFT contract, and the BFT contract arguments.
    /// The function returns a result indicating whether the operation was successful or not.

    pub async fn run(
        &self,
        identity: PathBuf,
        ic_host: &str,
        network: EvmNetwork,
        pk: H256,
        deploy_bft: bool,
    ) -> anyhow::Result<()> {
        match self {
            Commands::Deploy(deploy) => {
                deploy
                    .deploy_canister(identity, ic_host, network, pk, deploy_bft)
                    .await?
            }
            Commands::Reinstall(reinstall) => {
                reinstall
                    .reinstall_canister(identity, ic_host, network, pk, deploy_bft)
                    .await?
            }
            Commands::Upgrade(upgrade) => upgrade.upgrade_canister(identity, ic_host).await?,
        };

        Ok(())
    }
}

#[derive(Debug, Parser)]
pub struct BFTArgs {
    /// The address of the owner of the contract.
    #[arg(long, value_name = "OWNER")]
    owner: Option<H160>,

    /// The list of controllers for the contract.
    #[arg(long, value_name = "CONTROLLERS")]
    controllers: Option<Vec<H160>>,
}

impl BFTArgs {
    /// Deploy the BFT contract
    pub async fn deploy_bft(
        &self,
        network: EvmNetwork,
        canister_id: Principal,
        bridge: &Bridge,
        pk: H256,
        agent: &Agent,
    ) -> anyhow::Result<H160> {
        let contract_deployer = SolidityContractDeployer::new(network, pk);

        let expected_nonce = contract_deployer.get_nonce().await? + 2;

        let expected_address = contract_deployer.compute_fee_charge_address(expected_nonce)?;

        let canister_client = IcAgentClient::with_agent(canister_id, agent.clone());

        // Sleep for 1 second to allow the canister to be created
        tokio::time::sleep(Duration::from_secs(5)).await;

        let minter_address = canister_client
            .update::<_, BftResult<did::H160>>("get_bridge_canister_evm_address", ())
            .await??;

        info!("Minter address: {:x}", minter_address);

        let is_wrapped_side = bridge.is_wrapped_side();

        let bft_address = contract_deployer.deploy_bft(
            &minter_address.into(),
            &expected_address,
            is_wrapped_side,
            self.owner,
            &self.controllers,
        )?;

        contract_deployer.deploy_fee_charge(&[bft_address], Some(expected_address))?;

        Ok(bft_address)
    }
}

/// By default, the agent is configured to talk to the main
/// Internet Computer, and verifies responses using a hard-coded public key.
/// So we need to fetch the root key if the host is localhost.
pub(crate) async fn fetch_root_key(ic_host: &str, agent: &Agent) -> anyhow::Result<()> {
    if ic_host.contains("localhost") || ic_host.contains("127.0.0.1") {
        agent.fetch_root_key().await?;
    }

    Ok(())
}
