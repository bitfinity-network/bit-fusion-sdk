use std::time::Duration;

use anyhow::Context;
use bridge_client::Erc20BridgeClient;
use bridge_did::error::BftResult;
use bridge_did::evm_link::EvmLink;
use bridge_did::init::BtcBridgeConfig;
use candid::{Encode, Principal};
use clap::{Args, Subcommand};
use deploy::DeployCommands;
use eth_signer::sign_strategy::SigningStrategy;
use ethereum_types::{H160, H256};
use ic_agent::Agent;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_canister_client::{CanisterClient, IcAgentClient};
use reinstall::ReinstallCommands;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};
use upgrade::UpgradeCommands;

use crate::canister_ids::{CanisterIdsPath, CanisterType};
use crate::commands::wrap_token_type::WrapTokenType;
use crate::config;
use crate::contracts::{EvmNetwork, NetworkConfig, SolidityContractDeployer};
mod deploy;
mod reinstall;
mod upgrade;
mod wrap_token_type;

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

    #[command(subcommand)]
    Wrap(WrapTokenType),
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
        #[command(flatten, next_help_heading = "CkBTC connection")]
        connection: config::BtcBridgeConnection,
    },
    Erc20 {
        /// The configuration to use
        #[command(flatten)]
        init: config::InitBridgeConfig,
        /// Extra configuration for the ERC20 bridge
        #[command(name = "erc", flatten, next_help_heading = "Base EVM")]
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
    /// Returns the kind of bridge
    pub fn kind(&self) -> &'static str {
        match self {
            Bridge::Brc20 { .. } => "brc20-bridge",
            Bridge::Btc { .. } => "btc-bridge",
            Bridge::Erc20 { .. } => "erc20-bridge",
            Bridge::Icrc { .. } => "icrc2-bridge",
            Bridge::Rune { .. } => "rune-bridge",
        }
    }

    /// Initialize the raw argument for the bridge
    pub fn init_raw_arg(
        &self,
        owner: Principal,
        ic_host: &str,
        evm_network: EvmNetwork,
    ) -> anyhow::Result<Vec<u8>> {
        let arg = match &self {
            Bridge::Brc20 {
                config: init,
                brc20,
            } => {
                trace!("Preparing BRC20 bridge configuration");
                let init_data = init
                    .clone()
                    .into_bridge_init_data(owner, ic_host, evm_network);
                debug!("BRC20 Bridge Config : {:?}", init_data);
                let brc20_config = bridge_did::init::brc20::Brc20BridgeConfig::from(brc20.clone());
                Encode!(&init_data, &brc20_config)?
            }
            Bridge::Rune { init, rune } => {
                trace!("Preparing Rune bridge configuration");
                let init_data = init
                    .clone()
                    .into_bridge_init_data(owner, ic_host, evm_network);
                debug!("Init Bridge Config : {:?}", init_data);
                let rune_config = bridge_did::init::RuneBridgeConfig::from(rune.clone());
                debug!("Rune Bridge Config : {:?}", rune_config);
                Encode!(&init_data, &rune_config)?
            }
            Bridge::Icrc { config } => {
                trace!("Preparing ICRC bridge configuration");
                let config = config
                    .clone()
                    .into_bridge_init_data(owner, ic_host, evm_network);
                debug!("ICRC Bridge Config : {:?}", config);
                Encode!(&config)?
            }
            Bridge::Erc20 { init, erc } => {
                trace!("Preparing ERC20 bridge configuration");
                let signing_strategy = init.signing_key_id(ic_host).into();
                let init = init
                    .clone()
                    .into_bridge_init_data(owner, ic_host, evm_network);

                // Workaround for not depending on the `erc-20` crate
                #[derive(candid::CandidType)]
                struct EvmSettings {
                    pub evm_link: EvmLink,
                    pub signing_strategy: SigningStrategy,
                }

                let erc = EvmSettings {
                    evm_link: erc.clone().into(),
                    signing_strategy,
                };

                Encode!(&init, &erc)?
            }
            Bridge::Btc { config, connection } => {
                trace!("Preparing BTC bridge configuration");
                let connection = bridge_did::init::btc::BitcoinConnection::from(connection.clone());
                let init_data = config
                    .clone()
                    .into_bridge_init_data(owner, ic_host, evm_network);
                let config = BtcBridgeConfig {
                    network: connection,
                    init_data,
                };
                Encode!(&config)?
            }
        };

        Ok(arg)
    }

    pub async fn finalize(
        &self,
        bft_args: &BFTArgs,
        wrapped_network: EvmNetwork,
        bridge_principal: Principal,
        pk: H256,
        agent: &Agent,
    ) -> anyhow::Result<()> {
        match self {
            Self::Erc20 { erc, .. } => {
                let network = if let Some(url) = &erc.base_evm_url {
                    NetworkConfig {
                        custom_network: Some(url.clone()),
                        evm_network: EvmNetwork::Localhost,
                    }
                } else {
                    wrapped_network.into()
                };

                let bft_address = bft_args
                    .deploy_bft(network, bridge_principal, pk, &agent, false)
                    .await?;

                info!("Base BFT bridge deployed with address {bft_address:?}");

                let client = Erc20BridgeClient::new(IcAgentClient::with_agent(
                    bridge_principal,
                    agent.clone(),
                ));
                client
                    .set_base_bft_bridge_contract(&bft_address.bft_bridge.into())
                    .await?;

                info!("Bridge canister configured with base BFT bridge contract address");

                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl From<&Bridge> for CanisterType {
    fn from(value: &Bridge) -> Self {
        match value {
            Bridge::Brc20 { .. } => CanisterType::Brc20,
            Bridge::Rune { .. } => CanisterType::Rune,
            Bridge::Icrc { .. } => CanisterType::Icrc2,
            Bridge::Erc20 { .. } => CanisterType::Erc20,
            Bridge::Btc { .. } => CanisterType::Btc,
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
        identity: GenericIdentity,
        ic_host: &str,
        network: EvmNetwork,
        pk: H256,
        canister_ids_path: CanisterIdsPath,
    ) -> anyhow::Result<()> {
        match self {
            Commands::Deploy(deploy) => {
                deploy
                    .deploy_canister(identity, ic_host, network, pk, canister_ids_path)
                    .await?
            }
            Commands::Reinstall(reinstall) => {
                reinstall
                    .reinstall_canister(identity, ic_host, network, canister_ids_path)
                    .await?
            }
            Commands::Upgrade(upgrade) => upgrade.upgrade_canister(identity, ic_host).await?,
            Commands::Wrap(wrap_token_type) => wrap_token_type.wrap(network, pk).await?,
        };

        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct BFTArgs {
    /// The address of the owner of the contract.
    #[arg(long, value_name = "OWNER")]
    owner: Option<H160>,

    /// The list of controllers for the contract.
    #[arg(long, value_name = "CONTROLLERS")]
    controllers: Option<Vec<H160>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BftDeployedContracts {
    pub bft_bridge: H160,
    pub wrapped_token_deployer: H160,
}

impl BFTArgs {
    /// Deploy the BFT contract
    pub async fn deploy_bft(
        &self,
        network: NetworkConfig,
        canister_id: Principal,
        pk: H256,
        agent: &Agent,
        is_wrapped_side: bool,
    ) -> anyhow::Result<BftDeployedContracts> {
        info!("Deploying BFT bridge");

        let contract_deployer = SolidityContractDeployer::new(network, pk);

        let nonce_increment = match is_wrapped_side {
            true => 3,
            false => 2, // we don't deploy token deployer for base EVM
        };
        let expected_nonce = contract_deployer.get_nonce().await? + nonce_increment;

        debug!("Expected nonce: {expected_nonce}");
        let expected_fee_charge_address =
            contract_deployer.compute_fee_charge_address(expected_nonce)?;
        debug!("Expected address: {expected_fee_charge_address}");

        let canister_client = IcAgentClient::with_agent(canister_id, agent.clone());

        // Sleep for 1 second to allow the canister to be created
        tokio::time::sleep(Duration::from_secs(5)).await;

        let wrapped_token_deployer = if is_wrapped_side {
            contract_deployer.deploy_wrapped_token_deployer()?
        } else {
            H160::default()
        };

        let evm_address_method = if is_wrapped_side {
            "get_bridge_canister_evm_address"
        } else {
            "get_bridge_canister_base_evm_address"
        };
        let minter_address = canister_client
            .update::<_, BftResult<did::H160>>(evm_address_method, ())
            .await?
            .context("failed to get the bridge canister address")?;

        info!("Minter address: {:x}", minter_address);

        let bft_address = contract_deployer.deploy_bft(
            &minter_address.into(),
            &expected_fee_charge_address,
            &wrapped_token_deployer,
            is_wrapped_side,
            self.owner,
            &self.controllers,
        )?;

        contract_deployer.deploy_fee_charge(&[bft_address], Some(expected_fee_charge_address))?;

        info!("BFT bridge deployed successfully. Contract address: {bft_address}");

        Ok(BftDeployedContracts {
            bft_bridge: bft_address,
            wrapped_token_deployer,
        })
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
