use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

use alloy::hex::ToHexExt as _;
use alloy::primitives::{Address, B256, U256};
use anyhow::{Context, Result};
use bridge_did::evm_link::EvmLink;
use bridge_did::id256::Id256;
use bridge_utils::native::wait_for_tx;
use candid::Principal;
use clap::{Args, ValueEnum};
use did::BlockNumber;
use did::constant::EIP1559_INITIAL_BASE_FEE;
use eth_signer::LocalWallet;
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use ethereum_json_rpc_client::EthJsonRpcClient;
use ethereum_json_rpc_client::reqwest::ReqwestClient;
use tracing::{debug, error, info};

use crate::evm::{MAINNET_PRINCIPAL, TESTNET_PRINCIPAL, dfx_webserver_port};

const PRIVATE_KEY_ENV_VAR: &str = "PRIVATE_KEY";

pub(crate) const TESTNET_URL: &str = "https://testnet.bitfinity.network";
const MAINNET_URL: &str = "https://mainnet.bitfinity.network";

#[derive(Debug, Clone, Args)]
#[group(required = true, multiple = false)]
pub struct NetworkConfig {
    /// Internet Computer network to deploy the bridge canister to (possible values: `ic` | `localhost`; default: localhost)
    #[clap(value_enum, long)]
    pub bridge_network: IcNetwork,
    /// Custom network URL
    #[clap(long)]
    pub custom_network: Option<String>,
}

impl std::fmt::Display for NetworkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.custom_network.is_some() {
            return write!(f, "custom");
        }

        let network = match self.bridge_network {
            IcNetwork::Localhost => "localhost",
            IcNetwork::Ic => "ic",
        };

        write!(f, "{network}")
    }
}

impl From<IcNetwork> for NetworkConfig {
    fn from(value: IcNetwork) -> Self {
        Self {
            bridge_network: value,
            custom_network: None,
        }
    }
}

#[derive(Debug, Clone, Copy, strum::Display, ValueEnum, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum IcNetwork {
    Ic,
    Localhost,
}

pub struct SolidityContractDeployer {
    evm: EvmLink,
    network: NetworkConfig,
    wallet: LocalWallet,
}

impl SolidityContractDeployer {
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
    pub fn new(network: NetworkConfig, pk: B256, evm: EvmLink) -> Self {
        let wallet = LocalWallet::from_bytes(&pk).expect("invalid wallet PK value");
        Self {
            evm,
            network,
            wallet,
        }
    }

    /// Returns the evm URL based on the selected network.
    pub fn get_evm_url(&self) -> String {
        match &self.evm {
            EvmLink::Http(url) => url.to_string(),
            EvmLink::Ic(principal) => self.get_evm_url_from_principal(principal),
            EvmLink::EvmRpcCanister { .. } => {
                panic!("EVM RPC canister is not supported for contract deployment")
            }
        }
    }

    /// Returns the evm URL based on the given principal.
    fn get_evm_url_from_principal(&self, principal: &Principal) -> String {
        if let Some(custom_network) = &self.network.custom_network {
            custom_network.to_string()
        } else {
            match self.network.bridge_network {
                IcNetwork::Localhost => format!(
                    "http://127.0.0.1:{dfx_port}/?canisterId={principal}",
                    dfx_port = dfx_webserver_port(),
                ),
                IcNetwork::Ic
                    if principal
                        == &Principal::from_text(MAINNET_PRINCIPAL).expect("invalid principal") =>
                {
                    MAINNET_URL.to_string()
                }
                IcNetwork::Ic
                    if principal
                        == &Principal::from_text(TESTNET_PRINCIPAL).expect("invalid principal") =>
                {
                    TESTNET_URL.to_string()
                }
                _ => panic!("Invalid principal {principal}"),
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
        self.wallet.credential().to_bytes().encode_hex_with_prefix()
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
            &self.get_evm_url(),
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
        debug!("Environment variables: {env_vars:?}");

        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(format!(
                "cd {} && {} 2>&1",
                solidity_dir.display(),
                args.join(" ")
            ))
            .env(PRIVATE_KEY_ENV_VAR, self.pk())
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
            error!(
                "{} command failed. Stdout:\n{}\nStderr:\n{}",
                script_name, stdout, stderr
            );
            return Err(anyhow::anyhow!("{} command failed", script_name));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            debug!("{} command output: {}", script_name, stdout);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Extracts the address from the output.
    ///
    /// This is a helper function that extracts the address from the output.
    fn extract_address_from_output(output: &str, prefix: &str) -> Result<Address> {
        let address = output
            .lines()
            .find(|line| line.contains(prefix))
            .and_then(|line| line.split(':').nth(1))
            .map(str::trim)
            .context(format!("Failed to extract {} address", prefix))?;

        Address::from_str(address).context(format!("Invalid {} address", prefix))
    }

    /// Deploys the BTF contract.
    pub fn deploy_btf(
        &self,
        minter_address: &Address,
        fee_charge_address: &Address,
        wrapped_token_deployer_address: &Address,
        is_wrapped_side: bool,
        owner: Option<Address>,
        controllers: &Option<Vec<Address>>,
    ) -> Result<Address> {
        info!("Deploying BTF contract");

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
                .map(Address::encode_hex_upper_with_prefix)
                .collect::<Vec<String>>()
                .join(",");
            env_vars
                .into_iter()
                .chain(vec![("CONTROLLERS", controllers_str)])
                .collect()
        } else {
            env_vars
        };

        let output = self.execute_forge_script("DeployBTF.s.sol", env_vars)?;
        Self::extract_address_from_output(&output, "Proxy address:")
    }

    /// Deploys the WrappedTokenDeployer contract.
    pub fn deploy_wrapped_token_deployer(&self) -> Result<Address> {
        info!("Deploying WrappedTokenDeployer contract");
        let output = self.execute_forge_script("DeployWrappedTokenDeployer.s.sol", vec![])?;
        Self::extract_address_from_output(&output, "WrappedTokenDeployer address:")
    }

    /// Deploys the FeeCharge contract.
    pub fn deploy_fee_charge(
        &self,
        bridges: &[Address],
        expected_address: Option<Address>,
    ) -> Result<Address> {
        info!("Deploying Fee Charge contract");

        let bridges = bridges
            .iter()
            .map(Address::encode_hex_upper_with_prefix)
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
        btf_bridge: &Address,
        name: &str,
        symbol: &str,
        decimals: u8,
        base_token_id: Id256,
    ) -> Result<Address> {
        info!("Deploying Wrapped ERC20 contract");

        let env_vars = vec![
            ("BTF_BRIDGE", btf_bridge.encode_hex_with_prefix()),
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
    pub fn compute_fee_charge_address(&self, nonce: u64) -> Result<Address> {
        let deployer = self.wallet.address();
        let contract_address = bridge_utils::get_contract_address(deployer, U256::from(nonce));
        Ok(contract_address)
    }

    fn rpc_client(&self) -> anyhow::Result<EthJsonRpcClient<ReqwestClient>> {
        let url = self.get_evm_url();
        let reqwest_client = reqwest::ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()?;
        let client = EthJsonRpcClient::new(ReqwestClient::new_with_client(
            url.to_string(),
            reqwest_client,
        ));

        Ok(client)
    }

    /// Returns the nonce of the deployer.
    pub async fn get_nonce(&self) -> Result<u64> {
        let network = self.get_evm_url();
        info!("Requesting nonce for network {network}");

        let client = self.rpc_client()?;
        let address = self.wallet.address();
        let nonce = client
            .get_transaction_count(address.into(), BlockNumber::Latest)
            .await?;

        info!("Got nonce value for network {network}: {nonce}");

        Ok(nonce)
    }

    pub async fn transfer_eth(&self, to: &Address, amount: u128) -> Result<()> {
        info!(
            "Transferring {amount} ETH tokens to address {}",
            to.encode_hex_with_prefix()
        );
        let client = self.rpc_client()?;

        let address = self.wallet.address();
        let nonce = client
            .get_transaction_count(address.into(), BlockNumber::Latest)
            .await?;
        let chain_id = client.get_chain_id().await?;

        let tx = TransactionBuilder {
            from: &address.into(),
            to: Some((*to).into()),
            nonce: nonce.into(),
            value: amount.into(),
            gas: 5_000_000u64.into(),
            gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
            input: vec![],
            signature: SigningMethod::SigningKey(self.wallet.credential()),
            chain_id,
        }
        .calculate_hash_and_build()
        .expect("failed to sign the transaction");

        let hash = client.send_raw_transaction(&tx.try_into()?).await?;
        wait_for_tx(&client, hash.into()).await?;

        Ok(())
    }
}
