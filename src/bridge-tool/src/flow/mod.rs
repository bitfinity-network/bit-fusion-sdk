use std::str::FromStr;
use std::time::{Duration, Instant};

use alloy::hex::ToHexExt as _;
use alloy::primitives::{Address, B256};
use alloy::rpc::types::TransactionRequest;
use alloy_sol_types::SolCall;
use anyhow::anyhow;
use bridge_client::Erc20BridgeClient;
use bridge_did::id256::Id256;
use bridge_did::operations::Erc20OpStage;
use bridge_utils::{BTFBridge, FeeCharge, WrappedToken};
use candid::Principal;
use clap::{Args, Parser, Subcommand};
use did::constant::EIP1559_INITIAL_BASE_FEE;
use did::{BlockNumber, U256};
use eth_signer::LocalWallet;
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use ethereum_json_rpc_client::EthJsonRpcClient;
use ethereum_json_rpc_client::reqwest::ReqwestClient;
use ic_agent::Agent;
use ic_agent::identity::AnonymousIdentity;
use ic_canister_client::IcAgentClient;
use rand::random;
use tracing::{error, info};

#[derive(Debug, Parser)]
pub struct DepositToken {
    /// Private key of the wallet to be used for EVM operations
    #[arg(short('p'), long, value_name = "PRIVATE_KEY", env)]
    private_key: B256,

    /// Arguments for the token to be deposited
    #[command(subcommand)]
    token_type: DepositTokenType,
}

#[derive(Debug, Parser)]
pub struct WithdrawToken {
    /// Private key of the wallet to be used for EVM operations
    #[arg(short('p'), long, value_name = "PRIVATE_KEY", env)]
    private_key: B256,

    /// Arguments for the token to be withdrawn
    #[command(subcommand)]
    token_type: WithdrawTokenType,
}

#[derive(Debug, Subcommand)]
pub enum DepositTokenType {
    Erc20(DepositErc20Args),
}

#[derive(Debug, Subcommand)]
pub enum WithdrawTokenType {
    Erc20(WithdrawErc20Args),
}

#[derive(Debug, Args)]
pub struct DepositErc20Args {
    /// Base side EVM (localhost, testnet, mainnet or http address)
    #[arg(long)]
    base_evm: String,

    /// Base side BTF bridge contract address
    #[arg(long)]
    base_btf: Address,

    /// Base token address
    #[arg(long)]
    base_token: Address,

    /// Wrapped side EVM (localhost, testnet, mainnet or http address)
    #[clap(long)]
    wrapped_evm: String,

    /// Wrapped side BTF bridge contract address
    #[arg(long)]
    wrapped_btf: Address,

    /// Wrapped token address
    #[arg(long)]
    wrapped_token: Address,

    /// HTTP address of the IC connection to be used
    #[arg(long)]
    ic_host: String,

    /// Principal of the bridge canister
    #[arg(long)]
    bridge_canister: Principal,

    /// Amount of tokens to be transferred
    #[arg(long)]
    amount: u128,

    /// Recipient of the wrapped tokens (if no set, caller wallet will be used)
    #[arg(short, long)]
    recipient: Option<Address>,
}

#[derive(Debug, Args)]
pub struct WithdrawErc20Args {
    /// Base side EVM (localhost, testnet, mainnet or http address)
    #[arg(long)]
    base_evm: String,

    /// Base side BTF bridge contract address
    #[arg(long)]
    base_btf: Address,

    /// Base token address
    #[arg(long)]
    base_token: Address,

    /// Wrapped side EVM (localhost, testnet, mainnet or http address)
    #[clap(long)]
    wrapped_evm: String,

    /// Wrapped side BTF bridge contract address
    #[arg(long)]
    wrapped_btf: Address,

    /// Wrapped token address
    #[arg(long)]
    wrapped_token: Address,

    /// HTTP address of the IC connection to be used
    #[arg(long)]
    ic_host: String,

    /// Principal of the bridge canister
    #[arg(long)]
    bridge_canister: Principal,

    /// Amount of tokens to be transferred
    #[arg(long)]
    amount: u128,

    /// Recipient of the base tokens (if no set, caller wallet will be used)
    #[arg(short, long)]
    recipient: Option<Address>,
}

type RpcClient = EthJsonRpcClient<ReqwestClient>;

struct Erc20BridgeFlow {
    wallet: LocalWallet,
    base_client: RpcClient,
    wrapped_client: RpcClient,
    base_btf: Address,
    base_token: Address,
    wrapped_btf: Address,
    wrapped_token: Address,
    ic_host: String,
    bridge_canister: Principal,
}

impl Erc20BridgeFlow {
    fn new_deposit(pk: B256, args: &DepositErc20Args) -> Self {
        let reqwest_client = reqwest::ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("failed to create reqwest client");
        let base_client = EthJsonRpcClient::new(ReqwestClient::new_with_client(
            Self::evm_url(&args.base_evm),
            reqwest_client.clone(),
        ));
        let wrapped_client = EthJsonRpcClient::new(ReqwestClient::new_with_client(
            Self::evm_url(&args.wrapped_evm),
            reqwest_client,
        ));
        let wallet = LocalWallet::from_bytes(&pk).expect("invalid wallet PK value");

        Self {
            wallet,
            base_client,
            wrapped_client,
            base_btf: args.base_btf,
            base_token: args.base_token,
            wrapped_btf: args.wrapped_btf,
            wrapped_token: args.wrapped_token,
            ic_host: args.ic_host.clone(),
            bridge_canister: args.bridge_canister,
        }
    }

    fn new_withdraw(pk: B256, args: &WithdrawErc20Args) -> Self {
        let reqwest_client = reqwest::ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("failed to create reqwest client");
        let base_client = EthJsonRpcClient::new(ReqwestClient::new_with_client(
            Self::evm_url(&args.base_evm),
            reqwest_client.clone(),
        ));
        let wrapped_client = EthJsonRpcClient::new(ReqwestClient::new_with_client(
            Self::evm_url(&args.wrapped_evm),
            reqwest_client,
        ));
        let wallet = LocalWallet::from_bytes(&pk).expect("invalid wallet PK value");

        Self {
            wallet,
            base_client,
            wrapped_client,
            base_btf: args.base_btf,
            base_token: args.base_token,
            wrapped_btf: args.wrapped_btf,
            wrapped_token: args.wrapped_token,
            ic_host: args.ic_host.clone(),
            bridge_canister: args.bridge_canister,
        }
    }

    fn evm_url(arg: &str) -> String {
        match arg {
            "localhost" => "http://localhost:8545".to_string(),
            "testnet" => "https://testnet.bitfinity.network".to_string(),
            "mainnet" => "https://mainnet.bitfinity.network".to_string(),
            v => v.to_string(),
        }
    }
}

impl DepositToken {
    pub async fn run(&self) -> anyhow::Result<()> {
        match &self.token_type {
            DepositTokenType::Erc20(erc20args) => {
                let flow = Erc20BridgeFlow::new_deposit(self.private_key, erc20args);
                flow.deposit(erc20args.amount, erc20args.recipient).await
            }
        }
    }
}

impl WithdrawToken {
    pub async fn run(&self) -> anyhow::Result<()> {
        match &self.token_type {
            WithdrawTokenType::Erc20(args) => {
                let flow = Erc20BridgeFlow::new_withdraw(self.private_key, args);
                flow.withdraw(args.amount, args.recipient).await
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum EvmSide {
    Base,
    Wrapped,
}

impl EvmSide {
    fn other(&self) -> Self {
        match self {
            EvmSide::Base => EvmSide::Wrapped,
            EvmSide::Wrapped => EvmSide::Base,
        }
    }
}

const FEE_APPROVE_AMOUNT: u128 = 10u128.pow(15);

impl Erc20BridgeFlow {
    async fn chain_id(&self, evm_side: EvmSide) -> anyhow::Result<u64> {
        let (client, _, _) = self.get_side(evm_side);
        client.get_chain_id().await
    }

    async fn deposit(&self, amount: u128, recipient: Option<Address>) -> anyhow::Result<()> {
        self.wrap(amount, recipient, EvmSide::Base).await
    }

    async fn withdraw(&self, amount: u128, recipient: Option<Address>) -> anyhow::Result<()> {
        self.wrap(amount, recipient, EvmSide::Wrapped).await
    }

    async fn wrap(
        &self,
        amount: u128,
        recipient: Option<Address>,
        side: EvmSide,
    ) -> anyhow::Result<()> {
        let recipient = recipient.unwrap_or_else(|| self.wallet.address());
        let memo = Self::generate_memo();

        self.approve_erc20(amount, side).await?;
        self.approve_fee(side.other(), FEE_APPROVE_AMOUNT).await?;
        self.burn_btf(side, amount, &recipient, memo).await?;
        self.track_operation(memo, side.other()).await
    }

    async fn get_fee_charge_address(&self, side: EvmSide) -> anyhow::Result<Address> {
        let input = BTFBridge::feeChargeContractCall {}.abi_encode();

        let (client, btf, _) = self.get_side(side);
        let result = client
            .eth_call(
                &TransactionRequest {
                    from: Some(self.wallet.address()),
                    to: Some((*btf).into()),
                    gas: None,
                    gas_price: None,
                    value: None,
                    input: input.into(),
                    nonce: None,
                    chain_id: None,
                    ..Default::default()
                },
                BlockNumber::Latest,
            )
            .await
            .expect("get fee charge address call failed");
        let address = Address::from_str(result.trim_start_matches("0x").trim_start_matches("0"))
            .unwrap_or_else(|_| panic!("Invalid response for fee charge address: {result}"));

        info!(
            "Fee charge address for BTF {} is {}",
            btf.encode_hex_with_prefix(),
            address.encode_hex_with_prefix()
        );

        Ok(address)
    }

    async fn approve_fee(&self, evm_side: EvmSide, amount: u128) -> anyhow::Result<()> {
        info!("Requesting fee charge approve");

        let (client, _, _) = self.get_side(evm_side);
        let fee_charge = self.get_fee_charge_address(evm_side).await?;

        let amount: U256 = amount.into();

        let nonce = client
            .get_transaction_count(self.wallet.address().into(), BlockNumber::Latest)
            .await?;
        let chain_id = self.chain_id(evm_side).await?;

        let input = FeeCharge::nativeTokenDepositCall {}.abi_encode();

        let approve_tx = TransactionBuilder {
            from: &self.wallet.address().into(),
            to: Some(fee_charge.into()),
            nonce: nonce.into(),
            value: amount,
            gas: 5_000_000u64.into(),
            gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
            input,
            signature: SigningMethod::SigningKey(self.wallet.credential()),
            chain_id,
        }
        .calculate_hash_and_build()
        .expect("failed to sign the transaction");

        let hash = client
            .send_raw_transaction(&approve_tx.try_into().expect("failed to convert tx"))
            .await
            .expect("Failed to send raw transaction");

        Self::wait_for_tx(client, hash.into()).await?;

        info!("Fee charge approved");
        Ok(())
    }

    fn generate_memo() -> [u8; 32] {
        let v: u128 = random();
        let mut memo = [0; 32];
        memo[0..16].copy_from_slice(&v.to_be_bytes());

        memo
    }

    async fn track_operation(&self, memo: [u8; 32], side: EvmSide) -> anyhow::Result<()> {
        info!("Tracking operation with memo {}", hex::encode(memo));

        let agent = Agent::builder()
            .with_identity(AnonymousIdentity)
            .with_url(&self.ic_host)
            .build()?;
        agent.fetch_root_key().await?;

        let client = Erc20BridgeClient::new(IcAgentClient::with_agent(self.bridge_canister, agent));

        const OPERATION_TIMEOUT: Duration = Duration::from_secs(60);
        const REQUEST_INTERVAL: Duration = Duration::from_secs(1);
        let timeout = Instant::now() + OPERATION_TIMEOUT;

        let (operation_id, mut curr_step) = loop {
            if Instant::now() > timeout {
                return Err(anyhow!(
                    "Operation was not created during {} secs",
                    OPERATION_TIMEOUT.as_secs()
                ));
            }

            let operation = client
                .get_operation_by_memo_and_user(memo, &self.wallet.address().into())
                .await?;

            if let Some((operation_id, curr_step)) = operation {
                break (operation_id, curr_step);
            };

            tokio::time::sleep(REQUEST_INTERVAL).await;
        };

        info!("Operation id is {operation_id}");

        let mut prev_stage = "".to_string();
        while Instant::now() < timeout {
            let result = client.get_operation_log(operation_id).await?;
            let Some(operation_log) = result else {
                return Err(anyhow!("Operation {operation_id} not found"));
            };

            let current_stage = match operation_log
                .log()
                .last()
                .expect("log doesn't contain any entries")
                .step_result
                .clone()
            {
                Ok(curr_step) => curr_step.stage.name(),
                Err(error_msg) => error_msg,
            };

            if current_stage != prev_stage {
                info!("Operation {operation_id}: {current_stage}");
                prev_stage = current_stage;
                curr_step = operation_log.current_step().clone();
            }

            if matches!(curr_step.stage, Erc20OpStage::TokenMintConfirmed(_)) {
                info!("Operation {operation_id} is completed successfully");
                return Ok(());
            }

            tokio::time::sleep(REQUEST_INTERVAL).await;
        }

        let (evm_client, _, _) = self.get_side(side);
        if let Erc20OpStage::WaitForMintConfirm {
            tx_hash: Some(tx_hash),
            ..
        } = curr_step.stage
        {
            let tx_result = Self::wait_for_tx(evm_client, tx_hash.clone().into()).await;
            error!(
                "Bridge canister mint transaction ( {} ) result: {tx_result:?}",
                tx_hash.to_hex_str()
            );
        }

        Err(anyhow!(
            "Operation did not complete during {} seconds",
            OPERATION_TIMEOUT.as_secs()
        ))
    }

    fn get_side(&self, evm_side: EvmSide) -> (&RpcClient, &Address, &Address) {
        match evm_side {
            EvmSide::Base => (&self.base_client, &self.base_btf, &self.base_token),
            EvmSide::Wrapped => (&self.wrapped_client, &self.wrapped_btf, &self.wrapped_token),
        }
    }

    async fn approve_erc20(&self, amount: u128, evm_side: EvmSide) -> anyhow::Result<()> {
        info!("Approving transfer of {amount} ERC20 tokens");

        let (client, btf_bridge, token) = self.get_side(evm_side);
        let input = WrappedToken::balanceOfCall {
            account: self.wallet.address().0.into(),
        }
        .abi_encode();

        let result = client
            .eth_call(
                &TransactionRequest {
                    from: Some(self.wallet.address()),
                    to: Some((*token).into()),
                    gas: None,
                    gas_price: None,
                    value: None,
                    input: input.into(),
                    nonce: None,
                    chain_id: None,
                    ..Default::default()
                },
                BlockNumber::Latest,
            )
            .await
            .expect("balance call failed");
        let balance = u128::from_str_radix(result.trim_start_matches("0x"), 16)
            .expect("Failed to decode balance response");

        info!("Current token balance: {balance}");
        if balance < amount {
            return Err(anyhow!(
                "Balance ({balance}) is less then requested transfer amount ({amount})"
            ));
        }

        let amount: U256 = amount.into();

        let input = WrappedToken::approveCall {
            spender: btf_bridge.0.into(),
            value: amount.clone().into(),
        }
        .abi_encode();
        let nonce = client
            .get_transaction_count(self.wallet.address().into(), BlockNumber::Latest)
            .await?;
        let chain_id = client.get_chain_id().await?;

        let approve_tx = TransactionBuilder {
            from: &self.wallet.address().into(),
            to: Some((*token).into()),
            nonce: nonce.into(),
            value: 0u64.into(),
            gas: 5_000_000u64.into(),
            gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
            input,
            signature: SigningMethod::SigningKey(self.wallet.credential()),
            chain_id,
        }
        .calculate_hash_and_build()
        .expect("failed to sign the transaction");

        let hash = client
            .send_raw_transaction(&approve_tx.try_into().expect("failed to convert tx"))
            .await
            .expect("Failed to send raw transaction");

        Self::wait_for_tx(client, hash.into()).await?;

        info!("Erc20 transfer approved");
        Ok(())
    }

    async fn wait_for_tx(client: &RpcClient, hash: B256) -> anyhow::Result<Vec<u8>> {
        Ok(bridge_utils::native::wait_for_tx(client, hash).await?)
    }

    async fn burn_btf(
        &self,
        evm_side: EvmSide,
        amount: u128,
        recipient: &Address,
        memo: [u8; 32],
    ) -> anyhow::Result<B256> {
        info!("Requesting BTF burn with amount {amount} ");

        let (client, btf, from_token) = self.get_side(evm_side);
        let (_, _, to_token) = self.get_side(evm_side.other());

        let to_chain_id = self.chain_id(evm_side.other()).await?;
        let to_token_id = Id256::from_evm_address(&(*to_token).into(), to_chain_id as u32);

        let recipient_id = Id256::from_evm_address(&(*recipient).into(), to_chain_id as u32);
        let recipient = recipient_id.0;

        let amount: U256 = amount.into();
        let input = BTFBridge::burnCall {
            amount: amount.into(),
            fromERC20: from_token.0.into(),
            toTokenID: alloy_sol_types::private::FixedBytes::from_slice(&to_token_id.0),
            recipientID: alloy_sol_types::private::Bytes::copy_from_slice(&recipient),
            memo: alloy_sol_types::private::FixedBytes(memo),
        }
        .abi_encode();

        let nonce = client
            .get_transaction_count(self.wallet.address().into(), BlockNumber::Latest)
            .await?;
        let chain_id = client.get_chain_id().await?;
        let burn_tx = TransactionBuilder {
            from: &self.wallet.address().into(),
            to: Some((*btf).into()),
            nonce: nonce.into(),
            value: 0u64.into(),
            gas: 5_000_000u64.into(),
            gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
            input,
            signature: SigningMethod::SigningKey(self.wallet.credential()),
            chain_id,
        }
        .calculate_hash_and_build()
        .expect("failed to sign the transaction");

        let hash = client
            .send_raw_transaction(&burn_tx.try_into().expect("failed to convert tx"))
            .await
            .expect("Failed to send raw transaction");

        info!("BTF burn transaction sent: 0x{}", hash.to_hex_str());

        Ok(hash.into())
    }
}
