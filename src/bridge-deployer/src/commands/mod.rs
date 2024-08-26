use std::path::PathBuf;

use clap::Subcommand;
use deploy::DeployCommands;
use ethereum_types::H256;
use reinstall::ReinstallCommands;
use serde::{Deserialize, Serialize};
use upgrade::UpgradeCommands;

use crate::canister_manager::CanisterManager;
use crate::config;
use crate::contracts::EvmNetwork;

mod deploy;
mod reinstall;
mod upgrade;

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
    List,
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

impl Commands {
    pub async fn run(
        &self,
        identity: PathBuf,
        ic_host: &str,
        canister_manager: &mut CanisterManager,
        deploy_bft: bool,
        network: EvmNetwork,
        pk: H256,
    ) -> anyhow::Result<()> {
        match self {
            Commands::Deploy(deploy) => {
                deploy
                    .deploy_canister(identity, ic_host, canister_manager, deploy_bft, network, pk)
                    .await?
            }
            Commands::Reinstall(reinstall) => {
                reinstall
                    .reinstall_canister(identity, ic_host, canister_manager)
                    .await?
            }
            Commands::Upgrade(upgrade) => {
                upgrade
                    .upgrade_canister(identity, ic_host, canister_manager)
                    .await?
            }
            _ => unimplemented!(),
        };

        Ok(())
    }
}
