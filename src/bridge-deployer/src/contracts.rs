use anyhow::{Context, Result};
use clap::ValueEnum;
use eth_signer::{Signer, Wallet};
use ethereum_types::H256;
use ethers_core::k256::ecdsa::SigningKey;
use ethers_core::types::H160;
use std::process::Command;
use tracing::{error, info};

#[derive(Debug, Clone, Copy, strum::Display, ValueEnum)]
pub enum EvmNetwork {
    Local,
    Testnet,
    Mainnet,
}

pub struct ContractDeployer<'a> {
    network: EvmNetwork,
    wallet: Wallet<'a, SigningKey>,
}

impl ContractDeployer<'_> {
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
        is_wrapped_side: bool,
    ) -> Result<String> {
        info!("Deploying BFT contract");
        let output = Command::new("npx")
            .args([
                "hardhat",
                "deploy-bft",
                "--network",
                &self.network.to_string(),
                "--minter-address",
                &minter_address.to_string(),
                "--fee-charge-address",
                &fee_charge_address.to_string(),
                "--is-wrapped-side",
                &is_wrapped_side.to_string(),
            ])
            .output()
            .context("Failed to execute deploy-bft command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("BFT deployment failed: {}", stderr);
            return Err(anyhow::anyhow!("BFT deployment failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        info!("BFT deployment output: {}", stdout);

        // Extract the proxy address from the output
        let proxy_address = stdout
            .lines()
            .find(|line| line.starts_with("BFT deployed to:"))
            .and_then(|line| line.split(':').nth(1))
            .map(str::trim)
            .context("Failed to extract BFT proxy address")?;

        Ok(proxy_address.to_string())
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
        nonce: u64,
        expected_address: Option<&str>,
    ) -> Result<String> {
        info!("Deploying Fee Charge contract");
        let binding = bridges
            .iter()
            .map(H160::to_string)
            .collect::<Vec<String>>()
            .join(",");
        let nonce = nonce.to_string();
        let network = self.network.to_string();
        let mut args = vec![
            "hardhat",
            "deploy-fee-charge",
            "--network",
            &network,
            "--bridges",
            &binding,
            "--nonce",
            &nonce,
        ];

        if let Some(addr) = expected_address {
            args.push("--expected-address");
            args.push(addr);
        }

        let output = Command::new("npx")
            .args(&args)
            .output()
            .context("Failed to execute deploy-fee-charge command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Fee Charge deployment failed: {}", stderr);
            return Err(anyhow::anyhow!("Fee Charge deployment failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        info!("Fee Charge deployment output: {}", stdout);

        // Extract the fee charge address from the output
        let fee_charge_address = stdout
            .lines()
            .find(|line| line.starts_with("Fee charge address:"))
            .and_then(|line| line.split(':').nth(1))
            .map(str::trim)
            .context("Failed to extract Fee Charge address")?;

        Ok(fee_charge_address.to_string())
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
}
