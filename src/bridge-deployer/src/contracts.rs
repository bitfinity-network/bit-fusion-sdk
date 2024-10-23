use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

use anyhow::{Context, Result};
use bridge_did::id256::Id256;
use clap::{Args, ValueEnum};
use eth_signer::{Signer, Wallet};
use ethereum_json_rpc_client::reqwest::ReqwestClient;
use ethereum_json_rpc_client::EthJsonRpcClient;
use ethereum_types::H256;
use ethers_core::k256::ecdsa::SigningKey;
use ethers_core::types::{BlockNumber, H160};
use ethers_core::utils::hex::ToHexExt;
use tracing::{debug, info};

const LOCALHOST_URL: &str = "http://127.0.0.1:8545";
pub(crate) const TESTNET_URL: &str = "https://testnet.bitfinity.network";
const MAINNET_URL: &str = "https://mainnet.bitfinity.network";

#[derive(Debug, Clone, Args)]
#[group(required = true, multiple = false)]
pub struct NetworkConfig {
    /// EVM network to deploy the contract to (e.g. "mainnet", "testnet", "local")
    #[clap(value_enum, long)]
    pub evm_network: EvmNetwork,
    /// Custom network URL
    pub custom_network: Option<String>,
}

impl std::fmt::Display for NetworkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.custom_network.is_some() {
            return write!(f, "custom");
        }

        let network = match self.evm_network {
            EvmNetwork::Localhost => "localhost",
            EvmNetwork::Testnet => "testnet",
            EvmNetwork::Mainnet => "mainnet",
        };

        write!(f, "{network}")
    }
}

impl From<EvmNetwork> for NetworkConfig {
    fn from(value: EvmNetwork) -> Self {
        Self {
            evm_network: value,
            custom_network: None,
        }
    }
}

#[derive(Debug, Clone, Copy, strum::Display, ValueEnum, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum EvmNetwork {
    Localhost,
    Testnet,
    Mainnet,
}

pub struct SolidityContractDeployer<'a> {
    network: NetworkConfig,
    wallet: Wallet<'a, SigningKey>,
}

impl SolidityContractDeployer<'_> {
    /// Creates a new `ContractDeployer` instance with the given network and private key.
    ///
    /// # Arguments
    ///
    /// * `network` - The network to use for contract deployment.
    /// * `pk` - The private key to use for signing transactions.
    ///
    /// # Returns
    ///
    /// A new `ContractDeployer` instance.
    pub fn new(network: NetworkConfig, pk: H256) -> Self {
        let wallet = Wallet::from_bytes(pk.as_bytes()).expect("invalid wallet PK value");
        Self { network, wallet }
    }

    /// Returns the network URL based on the selected network.
    pub fn get_network_url(&self) -> &str {
        if let Some(custom_network) = &self.network.custom_network {
            custom_network
        } else {
            match self.network.evm_network {
                EvmNetwork::Localhost => LOCALHOST_URL,
                EvmNetwork::Testnet => TESTNET_URL,
                EvmNetwork::Mainnet => MAINNET_URL,
            }
        }
    }

    /// Returns the path to the solidity directory.
    pub fn solidity_dir(&self) -> PathBuf {
        std::env::current_dir()
            .context("Failed to get current directory")
            .expect("Failed to get current directory")
            .join("solidity")
    }

    /// Returns the private key of the deployer.
    pub fn pk(&self) -> String {
        self.wallet.signer().to_bytes().encode_hex_with_prefix()
    }

    /// Returns the address of the deployer.
    pub fn sender(&self) -> String {
        self.wallet.address().encode_hex_with_prefix()
    }

    /// Executes a forge script with the given environment variables.
    ///
    /// This is a helper function that executes a forge script with the given environment variables.
    fn execute_forge_script(
        &self,
        script_name: &str,
        env_vars: Vec<(&str, String)>,
    ) -> Result<String> {
        let solidity_dir = self.solidity_dir();
        let script_dir = solidity_dir.join("script").join(script_name);

        let args = [
            "forge",
            "script",
            "--broadcast",
            "-v",
            script_dir.to_str().expect("Invalid solidity dir"),
            "--rpc-url",
            self.get_network_url(),
            "--private-key",
            &self.pk(),
            "--sender",
            &self.sender(),
            "--slow",
        ];

        debug!(
            "Executing command: sh -c cd {} && {}",
            solidity_dir.display(),
            args.join(" ")
        );

        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(format!(
                "cd {} && {} 2>&1",
                solidity_dir.display(),
                args.join(" ")
            ))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env_vars {
            command.env(key, value);
        }

        let output = command
            .output()
            .context(format!("Failed to execute {} command", script_name))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            eprintln!(
                "{} command failed. Stdout:\n{}\nStderr:\n{}",
                script_name, stdout, stderr
            );
            return Err(anyhow::anyhow!("{} command failed", script_name));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("{} command output: {}", script_name, stdout);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Extracts the address from the output.
    ///
    /// This is a helper function that extracts the address from the output.
    fn extract_address_from_output(output: &str, prefix: &str) -> Result<H160> {
        let address = output
            .lines()
            .find(|line| line.contains(prefix))
            .and_then(|line| line.split(':').nth(1))
            .map(str::trim)
            .context(format!("Failed to extract {} address", prefix))?;

        H160::from_str(address).context(format!("Invalid {} address", prefix))
    }

    /// Deploys the BFT contract.
    pub fn deploy_bft(
        &self,
        minter_address: &H160,
        fee_charge_address: &H160,
        wrapped_token_deployer_address: &H160,
        is_wrapped_side: bool,
        owner: Option<H160>,
        controllers: &Option<Vec<H160>>,
    ) -> Result<H160> {
        info!("Deploying BFT contract");

        let env_vars = vec![
            ("MINTER_ADDRESS", minter_address.encode_hex_with_prefix()),
            (
                "FEE_CHARGE_ADDRESS",
                fee_charge_address.encode_hex_with_prefix(),
            ),
            (
                "WRAPPED_TOKEN_DEPLOYER",
                wrapped_token_deployer_address.encode_hex_with_prefix(),
            ),
            ("IS_WRAPPED_SIDE", is_wrapped_side.to_string()),
        ];

        let env_vars = if let Some(owner) = owner {
            env_vars
                .into_iter()
                .chain(vec![("OWNER", owner.encode_hex_with_prefix())])
                .collect()
        } else {
            env_vars
        };

        let env_vars = if let Some(controllers) = controllers {
            let controllers_str = controllers
                .iter()
                .map(H160::encode_hex_upper_with_prefix)
                .collect::<Vec<String>>()
                .join(",");
            env_vars
                .into_iter()
                .chain(vec![("CONTROLLERS", controllers_str)])
                .collect()
        } else {
            env_vars
        };

        let output = self.execute_forge_script("DeployBFT.s.sol", env_vars)?;
        Self::extract_address_from_output(&output, "Proxy address:")
    }

    /// Deploys the WrappedTokenDeployer contract.
    pub fn deploy_wrapped_token_deployer(&self) -> Result<H160> {
        info!("Deploying WrappedTokenDeployer contract");
        let output = self.execute_forge_script("DeployWrappedTokenDeployer.s.sol", vec![])?;
        Self::extract_address_from_output(&output, "WrappedTokenDeployer address:")
    }

    /// Deploys the FeeCharge contract.
    pub fn deploy_fee_charge(
        &self,
        bridges: &[H160],
        expected_address: Option<H160>,
    ) -> Result<H160> {
        info!("Deploying Fee Charge contract");

        let bridges = bridges
            .iter()
            .map(H160::encode_hex_upper_with_prefix)
            .collect::<Vec<String>>()
            .join(",");

        let mut env_vars = vec![("BRIDGES", bridges)];

        if let Some(addr) = expected_address {
            env_vars.push(("EXPECTED_ADDRESS", addr.encode_hex_upper_with_prefix()));
        }

        let output = self.execute_forge_script("DeployFeeCharge.s.sol", env_vars)?;
        Self::extract_address_from_output(&output, "Fee charge address:")
    }

    /// Deploys the WrappedToken contract.
    pub fn deploy_wrapped_token(
        &self,
        bft_bridge: &H160,
        name: &str,
        symbol: &str,
        decimals: u8,
        base_token_id: Id256,
    ) -> Result<H160> {
        info!("Deploying Wrapped ERC20 contract");

        let env_vars = vec![
            ("BFT_BRIDGE", bft_bridge.encode_hex_with_prefix()),
            ("NAME", name.to_string()),
            ("SYMBOL", symbol.to_string()),
            ("DECIMALS", decimals.to_string()),
            ("BASE_TOKEN_ID", base_token_id.0.encode_hex_with_prefix()),
        ];

        let output = self.execute_forge_script("DeployWrappedToken.s.sol", env_vars)?;

        Self::extract_address_from_output(&output, "ERC20 deployed at:")
    }

    /// Computes the address of the fee charge contract based on the deployer's address and the given nonce.
    ///
    /// # Arguments
    ///
    /// * `nonce` - The nonce to use for computing the contract address.
    ///
    /// # Returns
    ///
    /// The computed fee charge contract address.
    pub fn compute_fee_charge_address(&self, nonce: u64) -> Result<H160> {
        let deployer = self.wallet.address();
        let contract_address = ethers_core::utils::get_contract_address(deployer, nonce);
        Ok(contract_address)
    }

    /// Returns the nonce of the deployer.
    pub async fn get_nonce(&self) -> Result<u64> {
        let url = self.get_network_url();

        debug!("Requesting nonce with EVM url: {url}");

        let client = EthJsonRpcClient::new(ReqwestClient::new(url.to_string()));
        let address = self.wallet.address();
        let nonce = client
            .get_transaction_count(address, BlockNumber::Latest)
            .await?;

        info!("Got nonce value: {nonce}");

        Ok(nonce)
    }
}
