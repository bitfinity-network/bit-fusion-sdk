use std::path::PathBuf;

use bridge_did::init::btc::WrappedTokenConfig;
use candid::{Encode, Principal};
use clap::Parser;
use ethereum_types::{H160, H256};
use ic_agent::Identity;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use ic_utils::interfaces::{ManagementCanister, WalletCanister};
use tracing::{debug, info, trace};

use super::{BFTArgs, Bridge};
use crate::commands::BftDeployedContracts;
use crate::contracts::{EvmNetwork, SolidityContractDeployer};

/// The default number of cycles to deposit to the canister
const DEFAULT_CYCLES: u128 = 2_000_000_000_000;

const BTC_ERC20_NAME: [u8; 32] = [
    b'B', b'i', b't', b'c', b'o', b'i', b'n', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
];
const BTC_ERC20_SYMBOL: [u8; 16] = [b'B', b'T', b'C', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const BTC_ERC20_DECIMALS: u8 = 10;

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
            let BftDeployedContracts {
                bft_bridge,
                wrapped_token_deployer,
            } = self
                .bft_args
                .deploy_bft(network, canister_id, &self.bridge_type, pk, &agent)
                .await?;

            info!("BFT bridge deployed successfully with {bft_bridge}; wrapped_token_deployer: {wrapped_token_deployer}");
            println!("BFT bridge deployed with address {bft_bridge}; wrapped_token_deployer: {wrapped_token_deployer}");

            // If the bridge type is BTC, we also deploy the Token contract for wrapped BTC
            if matches!(&self.bridge_type, Bridge::Btc { .. }) {
                info!("Deploying wrapped BTC contract");
                let wrapped_btc_addr =
                    self.deploy_wrapped_btc(network, pk, &wrapped_token_deployer)?;

                info!("Wrapped BTC contract deployed successfully with {wrapped_btc_addr}");
                println!("Wrapped BTC contract deployed with address {wrapped_btc_addr}");

                info!("Configuring BTC wrapped token on the BTC bridge");
                self.configure_btc_wrapped_token(&agent, &canister_id, wrapped_btc_addr)
                    .await?;
            }
        }

        info!("Canister deployed successfully");

        println!(
            "Canister {canister_type} deployed with ID {canister_id}",
            canister_type = self.bridge_type.kind()
        );

        Ok(())
    }

    /// Deploys the wrapped BTC contract.
    fn deploy_wrapped_btc(
        &self,
        network: EvmNetwork,
        pk: H256,
        wrapped_token_deployer: &H160,
    ) -> anyhow::Result<H160> {
        let contract_deployer = SolidityContractDeployer::new(network, pk);

        contract_deployer.deploy_wrapped_token(
            wrapped_token_deployer,
            String::from_utf8_lossy(&BTC_ERC20_NAME).as_ref(),
            String::from_utf8_lossy(&BTC_ERC20_SYMBOL).as_ref(),
            BTC_ERC20_DECIMALS,
        )
    }

    /// Configure BTC wrapped token on the BTC bridge
    async fn configure_btc_wrapped_token(
        &self,
        agent: &ic_agent::Agent,
        principal: &Principal,
        wrapped_token: H160,
    ) -> anyhow::Result<()> {
        let config = WrappedTokenConfig {
            token_address: wrapped_token.into(),
            token_name: BTC_ERC20_NAME,
            token_symbol: BTC_ERC20_SYMBOL,
            decimals: BTC_ERC20_DECIMALS,
        };

        let args = Encode!(&config)?;

        agent
            .update(principal, "admin_configure_wrapped_token")
            .with_arg(args)
            .call_and_wait()
            .await?;

        Ok(())
    }
}
