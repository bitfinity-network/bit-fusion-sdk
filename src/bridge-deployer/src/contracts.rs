use std::process::{Command, Stdio};
use std::str::FromStr;

use anyhow::{Context, Result};
use clap::ValueEnum;
use eth_signer::{Signer, Wallet};
use ethereum_json_rpc_client::reqwest::ReqwestClient;
use ethereum_json_rpc_client::EthJsonRpcClient;
use ethereum_types::H256;
use ethers_core::k256::ecdsa::SigningKey;
use ethers_core::types::{BlockNumber, H160};
use ethers_core::utils::hex::ToHexExt;
use tracing::{debug, info};

#[derive(Debug, Clone, Copy, strum::Display, ValueEnum)]
#[strum(serialize_all = "snake_case")]
pub enum EvmNetwork {
    Localhost,
    Testnet,
    Mainnet,
}

pub struct SolidityContractDeployer<'a> {
    network: EvmNetwork,
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
    pub fn new(network: EvmNetwork, pk: H256) -> Self {
        let wallet = Wallet::from_bytes(pk.as_bytes()).expect("invalid wallet PK value");

        Self { network, wallet }
    }

    pub fn get_network_url(&self) -> &'static str {
        match self.network {
            EvmNetwork::Localhost => "http://127.0.0.1:8545",
            EvmNetwork::Testnet => "https://testnet.bitfinity.network",
            EvmNetwork::Mainnet => "https://mainnet.bitfinity.network",
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

        let dir = std::env::current_dir()
            .context("Failed to get current directory")?
            .join("solidity");
        let binding = dir.display().to_string();
        let dir = binding.as_str();
        info!("Deploying Fee Charge contract in {}", dir);
        let pk = self.wallet.signer().to_bytes().encode_hex_with_prefix();

        let sender = self.wallet.address().encode_hex_with_prefix();
        let args = [
            "script",
            "--target-contract",
            "DeployBFTBridge",
            "--broadcast",
            "-v",
            dir,
            "--rpc-url",
            self.get_network_url(),
            "--private-key",
            &pk,
            "--sender",
            &sender,
        ];

        let dir = std::env::current_dir()
            .context("Failed to get current directory")?
            .join("solidity");
        let dir = dir.display();
        info!("Deploying Fee Charge contract in {}", dir);

        debug!(
            "Executing command: sh -c cd {} && forge {}",
            dir,
            args.join(" ")
        );

        let mut sh = Command::new("sh");
        let command = sh
            .arg("-c")
            .env("MINTER_ADDRESS", &minter_address)
            .env("FEE_CHARGE_ADDRESS", &fee_charge_address)
            .env("IS_WRAPPED_SIDE", &is_wrapped_side)
            .env("WRAPPED_TOKEN_DEPLOYER", &wrapped_token_deployer_address)
            .arg(format!("cd {} && forge {} 2>&1", dir, args.join(" ")))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(owner) = owner {
            command.env("OWNER", &owner);
        }

        if let Some(controllers) = &controllers {
            command.env("CONTROLLERS", controllers);
        }

        let output = command
            .output()
            .context("Failed to execute deploy-bft command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            eprintln!(
                "deploy-bft command failed. Stdout:\n{}\nStderr:\n{}",
                stdout, stderr
            );

            return Err(anyhow::anyhow!("deploy-bft command failed"));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("deploy-bft command output:\n{}", stdout);
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
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute deploy-wrapped-token-deployer command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            eprintln!(
                "deploy-wrapped-token-deployer command failed. Stdout:\n{}\nStderr:\n{}",
                stdout, stderr
            );

            return Err(anyhow::anyhow!(
                "deploy-wrapped-token-deployer command failed"
            ));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("deploy-wrapped-token-deployer command output:\n{}", stdout);
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
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute deploy-bft command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            eprintln!(
                "deploy-fee-charge command failed. Stdout:\n{}\nStderr:\n{}",
                stdout, stderr
            );

            return Err(anyhow::anyhow!("deploy-fee-charge command failed"));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("deploy-fee-charge command output:\n{}", stdout);
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
    pub async fn get_nonce(&self) -> Result<u64> {
        let url = self.get_network_url();

        let client = EthJsonRpcClient::new(ReqwestClient::new(url.to_string()));

        let address = self.wallet.address();
        let nonce = client
            .get_transaction_count(address, BlockNumber::Latest)
            .await?;

        Ok(nonce)
    }
}
