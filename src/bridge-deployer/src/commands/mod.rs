use std::path::PathBuf;

use bridge_did::error::BftResult;
use candid::{Encode, Principal};
use clap::{Parser, Subcommand};
use deploy::DeployCommands;
use ethereum_types::{H160, H256};
use ic_canister_client::CanisterClient;
use reinstall::ReinstallCommands;
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};
use upgrade::UpgradeCommands;

use crate::config;
use crate::contracts::{ContractDeployer, EvmNetwork};

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
    Rune {
        /// The configuration to use
        #[command(name = "config", flatten)]
        config: config::RuneBridgeConfig,
    },
    Icrc {
        /// The configuration to use
        #[command(name = "config", flatten)]
        config: config::InitBridgeConfig,
    },
    Erc20 {
        /// The configuration to use
        #[command(name = "config", flatten)]
        init: config::InitBridgeConfig,
        /// Extra configuration for the ERC20 bridge
        #[command(name = "erc", flatten)]
        erc: config::BaseEvmSettingsConfig,
    },
    Btc {
        /// The configuration to use
        #[command(name = "config", flatten)]
        config: config::InitBridgeConfig,
    },
}

impl Bridge {
    /// Initialize the raw argument for the bridge
    pub fn init_raw_arg(&self) -> anyhow::Result<Vec<u8>> {
        let arg = match &self {
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

        Ok(arg)
    }

    /// Returns if the bridge is wrapped side or not
    pub fn is_wrapped_side(&self) -> bool {
        match self {
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
        bft_args: BFTArgs,
    ) -> anyhow::Result<()> {
        match self {
            Commands::Deploy(deploy) => {
                deploy
                    .deploy_canister(identity, ic_host, network, pk, deploy_bft, bft_args)
                    .await?
            }
            Commands::Reinstall(reinstall) => {
                reinstall
                    .reinstall_canister(identity, ic_host, network, pk, deploy_bft, bft_args)
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
    ) -> anyhow::Result<H160> {
        let contract_deployer = ContractDeployer::new(network, pk);

        let expected_nonce = contract_deployer.get_nonce().await? + 2;

        let expected_address = contract_deployer.compute_fee_charge_address(expected_nonce)?;

        let canister_client = ic_canister_client::ic_client::IcCanisterClient::new(canister_id);

        let minter_address = canister_client
            .update::<_, BftResult<did::H160>>("get_bridge_canister_evm_address", ())
            .await??;

        let is_wrapped_side = bridge.is_wrapped_side();

        let bft_address = contract_deployer.deploy_bft(
            &minter_address.into(),
            &expected_address,
            is_wrapped_side,
            self.owner,
            &self.controllers,
        )?;

        contract_deployer.deploy_fee_charge(
            &[bft_address],
            expected_nonce,
            Some(expected_address.to_string().as_str()),
        )?;

        Ok(bft_address)
    }
}
