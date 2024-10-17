use std::process::{Command, Stdio};
use std::str::FromStr;

use anyhow::{Context, Result};
use candid::Principal;
use clap::ValueEnum;
use eth_signer::{Signer, Wallet};
use ethereum_json_rpc_client::reqwest::ReqwestClient;
use ethereum_json_rpc_client::EthJsonRpcClient;
use ethereum_types::H256;
use ethers_core::k256::ecdsa::SigningKey;
use ethers_core::types::{BlockNumber, H160};
use ethers_core::utils::hex::ToHexExt;
use tracing::{debug, error, info};

use crate::evm::dfx_webserver_port;

const PRIVATE_KEY_ENV_VAR: &str = "PRIVATE_KEY";
const LOCALHOST_URL_ENV_VAR: &str = "LOCALHOST_URL";

#[derive(Debug, Clone, Copy, strum::Display, ValueEnum)]
#[strum(serialize_all = "snake_case")]
pub enum EvmNetwork {
    Localhost,
    Testnet,
    Mainnet,
}

pub struct SolidityContractDeployer<'a> {
    network: EvmNetwork,
    evm_localhost_url: Option<String>,
    wallet: Wallet<'a, SigningKey>,
    private_key: H256,
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
    pub fn new(network: EvmNetwork, pk: H256, evm_localhost_url: Option<String>) -> Self {
        let wallet = Wallet::from_bytes(pk.as_bytes()).expect("invalid wallet PK value");

        Self {
            network,
            wallet,
            evm_localhost_url,
            private_key: pk,
        }
    }

    pub fn get_network_url(&self, evm: Principal) -> String {
        match self.network {
            EvmNetwork::Localhost => {
                format!(
                    "http://127.0.0.1:{dfx_port}/?canisterId={evm}",
                    dfx_port = dfx_webserver_port()
                )
            }
            EvmNetwork::Testnet => "https://testnet.bitfinity.network".to_string(),
            EvmNetwork::Mainnet => "https://mainnet.bitfinity.network".to_string(),
        }
    }

    /// Deploys the BFT contract with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `minter_address` - The address of the minter contract.
    /// * `fee_charge_address` - The address of the fee charge contract.
    /// * `is_wrapped_side` - A boolean indicating whether the BFT contract is for the wrapped side.
    ///
    /// # Returns
    ///
    /// The address of the deployed BFT contract.
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

        let network = self.network.to_string();
        let minter_address = minter_address.encode_hex_with_prefix();
        let fee_charge_address = fee_charge_address.encode_hex_with_prefix();
        let wrapped_token_deployer_address =
            wrapped_token_deployer_address.encode_hex_with_prefix();
        let is_wrapped_side = is_wrapped_side.to_string();
        let owner = owner.map(|o| o.encode_hex_with_prefix());
        let controllers = controllers.as_ref().map(|c| {
            c.iter()
                .map(H160::encode_hex_upper_with_prefix)
                .collect::<Vec<String>>()
                .join(",")
        });
        let mut args = vec![
            "hardhat",
            "deploy-bft",
            "--network",
            &network,
            "--minter-address",
            &minter_address,
            "--fee-charge-address",
            &fee_charge_address,
            "--wrapped-token-deployer-address",
            &wrapped_token_deployer_address,
            "--is-wrapped-side",
            &is_wrapped_side,
        ];

        if let Some(ref owner) = owner {
            args.push("--owner");
            args.push(owner);
        }

        if let Some(ref controllers) = controllers {
            args.push("--controllers");
            args.push(controllers);
        }

        let dir = std::env::current_dir()
            .context("Failed to get current directory")?
            .join("solidity");
        let dir = dir.display();
        info!("Deploying Fee Charge contract in {}", dir);

        debug!(
            "Executing command: sh -c cd {} && npx {}",
            dir,
            args.join(" ")
        );

        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("cd {} && npx {} 2>&1", dir, args.join(" ")))
            .env(PRIVATE_KEY_ENV_VAR, self.private_key_str())
            .env(
                LOCALHOST_URL_ENV_VAR,
                self.evm_localhost_url.as_deref().unwrap_or(""),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute deploy-bft command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            error!("deploy-bft command failed: {}", args.join(" "));
            error!(
                "deploy-bft command failed. Stdout:\n{}\nStderr:\n{}",
                stdout, stderr
            );

            return Err(anyhow::anyhow!("deploy-bft command failed"));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            debug!("deploy-bft command output:\n{}", stdout);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Extract the proxy address from the output
        let proxy_address = stdout
            .lines()
            .find(|line| line.starts_with("BFT deployed to:"))
            .and_then(|line| line.split(':').nth(1))
            .map(str::trim)
            .context("Failed to extract BFT proxy address")?;

        let address = H160::from_str(proxy_address).context("Invalid BFT proxy address")?;
        Ok(address)
    }

    /// Deploys WrappedTokenDeployer contract
    pub fn deploy_wrapped_token_deployer(&self) -> Result<H160> {
        info!("Deploying WrappedTokenDeployer contract");
        let network = self.network.to_string();

        let args = [
            "hardhat",
            "deploy-wrapped-token-deployer",
            "--network",
            &network,
        ];

        let dir = std::env::current_dir()
            .context("Failed to get current directory")?
            .join("solidity");

        let dir = dir.display();
        info!("Deploying Fee Charge contract in {}", dir);

        debug!(
            "Executing command: sh -c cd {} && npx {}",
            dir,
            args.join(" ")
        );

        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("cd {} && npx {} 2>&1", dir, args.join(" ")))
            .env(PRIVATE_KEY_ENV_VAR, self.private_key_str())
            .env(
                LOCALHOST_URL_ENV_VAR,
                self.evm_localhost_url.as_deref().unwrap_or(""),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute deploy-wrapped-token-deployer command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            error!(
                "deploy-wrapped-token-deployer command failed: {}",
                args.join(" ")
            );
            error!(
                "deploy-wrapped-token-deployer command failed. Stdout:\n{}\nStderr:\n{}",
                stdout, stderr
            );

            return Err(anyhow::anyhow!(
                "deploy-wrapped-token-deployer command failed"
            ));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            debug!("deploy-wrapped-token-deployer command output:\n{}", stdout);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Extract the fee charge address from the output
        let wrapped_token_deployer_address = stdout
            .lines()
            .find(|line| line.starts_with("WrappedTokenDeployer address:"))
            .and_then(|line| line.split(':').nth(1))
            .map(str::trim)
            .context("Failed to extract WrappedTokenDeployer address")?;

        let address = H160::from_str(wrapped_token_deployer_address)
            .context("Invalid WrappedTokenDeployer address")?;

        Ok(address)
    }

    /// Deploys the Fee Charge contract with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `bridges` - A list of bridge addresses to be associated with the Fee Charge contract.
    /// * `nonce` - The nonce to use for computing the contract address.
    /// * `expected_address` - An optional expected address for the deployed Fee Charge contract.
    ///
    /// # Returns
    ///
    /// The address of the deployed Fee Charge contract.
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
        let network = self.network.to_string();
        let expected_address = expected_address.map(|addr| addr.encode_hex_upper_with_prefix());

        let mut args = vec![
            "hardhat",
            "deploy-fee-charge",
            "--network",
            &network,
            "--bridges",
            &bridges,
        ];

        if let Some(ref addr) = expected_address {
            args.push("--expected-address");
            args.push(addr)
        }

        let dir = std::env::current_dir()
            .context("Failed to get current directory")?
            .join("solidity");

        let dir = dir.display();
        info!("Deploying Fee Charge contract in {}", dir);

        debug!(
            "Executing command: sh -c cd {} && npx {}",
            dir,
            args.join(" ")
        );

        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("cd {} && npx {} 2>&1", dir, args.join(" ")))
            .env(PRIVATE_KEY_ENV_VAR, self.private_key_str())
            .env(
                LOCALHOST_URL_ENV_VAR,
                self.evm_localhost_url.as_deref().unwrap_or(""),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute deploy-bft command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            error!("deploy-fee-charge command failed: {}", args.join(" "));
            error!(
                "deploy-fee-charge command failed. Stdout:\n{}\nStderr:\n{}",
                stdout, stderr
            );

            return Err(anyhow::anyhow!("deploy-fee-charge command failed"));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            debug!("deploy-fee-charge command output:\n{}", stdout);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Extract the fee charge address from the output
        let fee_charge_address = stdout
            .lines()
            .find(|line| line.starts_with("Fee charge address:"))
            .and_then(|line| line.split(':').nth(1))
            .map(str::trim)
            .context("Failed to extract Fee Charge address")?;

        let address = H160::from_str(fee_charge_address).context("Invalid Fee Charge address")?;

        Ok(address)
    }

    /// Deploy wrapped ERC20 on the wrapped token deployer contract
    pub fn deploy_wrapped_token(
        &self,
        wrapped_token_deployer: &H160,
        name: &str,
        symbol: &str,
        decimals: u8,
    ) -> Result<H160> {
        let owner = self.wallet.address();
        let network = self.network.to_string();
        let wrapped_token_deployer = wrapped_token_deployer.encode_hex_with_prefix();
        let owner = owner.encode_hex_with_prefix();
        let decimals = decimals.to_string();

        let args = vec![
            "hardhat",
            "deploy-wrapped-token",
            "--network",
            &network,
            "--wrapped-token-deployer",
            &wrapped_token_deployer,
            "--name",
            name,
            "--symbol",
            symbol,
            "--decimals",
            &decimals,
            "--owner",
            &owner,
        ];

        let dir = std::env::current_dir()
            .context("Failed to get current directory")?
            .join("solidity");

        let dir = dir.display();
        info!("Deploying ERC20 contract in {dir}");

        debug!(
            "Executing command: sh -c cd {dir} && npx {}",
            args.join(" ")
        );

        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("cd {} && npx {} 2>&1", dir, args.join(" ")))
            .env(PRIVATE_KEY_ENV_VAR, self.private_key_str())
            .env(
                LOCALHOST_URL_ENV_VAR,
                self.evm_localhost_url.as_deref().unwrap_or(""),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute deploy-wrapped-token command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            error!("deploy-wrapped-token command failed: {}", args.join(" "));
            error!(
                "deploy-wrapped-token command failed. Stdout:\n{}\nStderr:\n{}",
                stdout, stderr
            );

            return Err(anyhow::anyhow!("deploy-wrapped-token command failed"));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            debug!("deploy-wrapped-token command output:\n{}", stdout);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Extract the fee charge address from the output
        let fee_charge_address = stdout
            .lines()
            .find(|line| line.starts_with("ERC20 deployed at:"))
            .and_then(|line| line.split(':').nth(1))
            .map(str::trim)
            .context("Failed to extract ERC20 address")?;

        let address = H160::from_str(fee_charge_address).context("Invalid ERC20 address")?;

        Ok(address)
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

    /// Retrieves the nonce of the deployer's address.
    ///
    /// # Returns
    ///
    /// The nonce of the deployer's address.
    pub async fn get_nonce(&self, evm: Principal) -> Result<u64> {
        let url = self.get_network_url(evm);

        let client = EthJsonRpcClient::new(ReqwestClient::new(url.to_string()));

        let address = self.wallet.address();
        let nonce = client
            .get_transaction_count(address, BlockNumber::Latest)
            .await?;

        Ok(nonce)
    }

    /// Returns the private key as a string.
    fn private_key_str(&self) -> String {
        hex::encode(self.private_key.as_bytes())
    }
}
