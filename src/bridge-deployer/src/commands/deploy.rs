use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::bail;
use bridge_did::id256::Id256;
use bridge_did::init::btc::WrappedTokenConfig;
use candid::{Encode, Principal};
use clap::Parser;
use ethereum_types::{H160, H256};
use ic_agent::Agent;
use ic_canister_client::agent::identity::GenericIdentity;
use ic_utils::interfaces::management_canister::builders::InstallMode;
use tracing::info;

use super::{BFTArgs, Bridge};
use crate::bridge_deployer::BridgeDeployer;
use crate::canister_ids::{CanisterIds, CanisterIdsPath};
use crate::commands::BftDeployedContracts;
use crate::config::BtcBridgeConnection;
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

    /// Wallet canister ID that is used in the creation of canisters.
    ///
    /// If not set, default wallet of the currently active dfx identity will be used.
    #[arg(long, value_name = "WALLET_CANISTER", env)]
    wallet_canister: Option<Principal>,

    /// These are extra arguments for the BFT bridge.
    #[command(flatten, next_help_heading = "BFT Bridge deployment")]
    bft_args: BFTArgs,
}

impl DeployCommands {
    /// Deploys a canister with the specified configuration.
    pub async fn deploy_canister(
        &self,
        identity: GenericIdentity,
        ic_host: &str,
        network: EvmNetwork,
        pk: H256,
        canister_ids_path: CanisterIdsPath,
    ) -> anyhow::Result<()> {
        info!("Starting canister deployment");
        let mut canister_ids = CanisterIds::read_or_default(canister_ids_path);

        let agent = Agent::builder()
            .with_url(ic_host)
            .with_identity(identity)
            .build()?;

        super::fetch_root_key(ic_host, &agent).await?;
        let wallet_canister = self.get_wallet_canister(ic_host)?;

        let deployer = BridgeDeployer::create(agent.clone(), wallet_canister, self.cycles).await?;
        let canister_id = deployer
            .install_wasm(&self.wasm, &self.bridge_type, InstallMode::Install, network)
            .await?;

        println!("Canister deployed with ID {canister_id}",);

        info!("Deploying BFT bridge");
        let BftDeployedContracts {
            bft_bridge,
            wrapped_token_deployer,
        } = self
            .bft_args
            .deploy_bft(network.into(), canister_id, pk, &agent, true)
            .await?;

        info!("BFT bridge deployed successfully with {bft_bridge}; wrapped_token_deployer: {wrapped_token_deployer}");
        println!("BFT bridge deployed with address {bft_bridge}; wrapped_token_deployer: {wrapped_token_deployer}");

        // If the bridge type is BTC, we also deploy the Token contract for wrapped BTC
        if let Bridge::Btc { connection, .. } = &self.bridge_type {
            info!("Deploying wrapped BTC contract");
            let wrapped_btc_addr =
                self.deploy_wrapped_btc(network, pk, &bft_bridge, *connection)?;

            info!("Wrapped BTC contract deployed successfully with {wrapped_btc_addr}");
            println!("Wrapped BTC contract deployed with address {wrapped_btc_addr}");

            info!("Configuring BTC wrapped token on the BTC bridge");
            self.configure_btc_wrapped_token(&agent, &canister_id, wrapped_btc_addr)
                .await?;
        }

        // set principal in canister ids
        canister_ids.set((&self.bridge_type).into(), canister_id);

        // configure minter
        deployer.configure_minter(bft_bridge).await?;

        self.bridge_type
            .finalize(
                &self.bft_args,
                network,
                deployer.bridge_principal(),
                pk,
                &agent,
            )
            .await?;

        // write canister ids file
        canister_ids.write()?;

        info!("Canister deployed successfully");

        println!(
            "Canister {canister_type} deployed with ID {canister_id}",
            canister_type = self.bridge_type.kind(),
            canister_id = deployer.bridge_principal(),
        );

        Ok(())
    }

    /// Deploys the wrapped BTC contract.
    fn deploy_wrapped_btc(
        &self,
        network: EvmNetwork,
        pk: H256,
        bft_bridge: &H160,
        btc_connection: BtcBridgeConnection,
    ) -> anyhow::Result<H160> {
        let contract_deployer = SolidityContractDeployer::new(network.into(), pk);
        let base_token_id = Id256::from(btc_connection.ledger_principal());

        contract_deployer.deploy_wrapped_token(
            bft_bridge,
            String::from_utf8_lossy(&BTC_ERC20_NAME).as_ref(),
            String::from_utf8_lossy(&BTC_ERC20_SYMBOL).as_ref(),
            BTC_ERC20_DECIMALS,
            base_token_id,
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

    fn get_wallet_canister(&self, ic_host: &str) -> anyhow::Result<Principal> {
        if let Some(principal) = self.wallet_canister {
            return Ok(principal);
        }

        let mut command = Command::new("dfx");
        command.args(vec!["identity", "get-wallet"]);

        if ic_host.starts_with("httsp://ic0.app") {
            command.arg("--ic");
        }

        let result = command.stdout(Stdio::piped()).output()?;

        if !result.status.success() {
            bail!(
                "Failed to get wallet principal: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }

        let principal = Principal::from_text(&String::from_utf8(result.stdout)?.trim())?;
        Ok(principal)
    }
}
