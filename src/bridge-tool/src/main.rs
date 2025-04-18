use std::str::FromStr;
use std::time::Duration;

use alloy::primitives::{Address, B256};
use alloy_sol_types::{SolCall, SolConstructor};
use bridge_did::id256::Id256;
use bridge_did::reason::Icrc2Burn;
use bridge_utils::{BTFBridge, FeeCharge, UUPSProxy, WrappedToken, WrappedTokenDeployer};
use candid::{CandidType, Encode, IDLArgs, Principal, TypeEnv};
use clap::Parser;
use did::constant::EIP1559_INITIAL_BASE_FEE;
use did::{BlockNumber, H256, Transaction, TransactionReceipt, U256};
use eth_signer::LocalWallet;
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use evm_canister_client::EvmCanisterClient;
use ic_canister_client::IcAgentClient;
use tokio::time::Instant;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{Layer, filter};

use crate::flow::{DepositToken, WithdrawToken};

mod flow;

// This identity is only used to make the calls non-anonymous. No actual checks depend on this
// identity.
const IDENTITY_PATH: &str = "src/bridge-tool/identity.pem";

/// Some operations with BTF bridge.
#[derive(Parser, Debug)]
#[clap(version = "0.1")]
enum CliCommand {
    /// Create btf bridge contract.
    DeployBtfbridge(DeployBtfArgs),
    /// Create WrappedTokenDeployer contract.
    DeployWrappedTokenDeployer(DeployWrappedTokenDeployerArgs),
    /// Create wrapper token contract.
    CreateToken(CreateTokenArgs),
    /// Create a new ETH wallet and mint native tokens to it.
    CreateWallet(CreateWalletArgs),
    /// Burn wrapped BTC.
    BurnWrapped(BurnWrappedArgs),
    /// Return ETH wallet address.
    WalletAddress(WalletAddressArgs),
    /// Create btf bridge contract.
    DeployFeeCharge(DeployFeeChargeArgs),
    /// Returns expected contract address for the given parameters.
    ExpectedContractAddress(ExpectedContractAddress),
    /// Request ICRC2 deposit
    DepositIcrc(DepositIcrcArgs),
    /// Get wallet nonce
    GetNonce(GetNonceArgs),
    /// Run deposit flow (mint wrapped tokens from the base tokens)
    Deposit(DepositToken),
    /// Run withdrawal flow (recieve base tokens from the wrapped tokens)
    Withdraw(WithdrawToken),
}

#[derive(Debug, Parser)]
struct GetNonceArgs {
    /// Evm canister principal
    #[arg(long)]
    evm: Principal,
    #[arg(long)]
    /// PK of the EVM wallet to get nonce for
    wallet: Option<String>,

    /// IC host (uses local dfx deployment by default)
    #[arg(long)]
    ic_host: Option<String>,
}

#[derive(Debug, Parser)]
struct DepositIcrcArgs {
    /// Evm canister principal
    #[arg(long)]
    evm: Principal,

    /// EVM address of the BTF bridge
    #[arg(long)]
    btf_bridge: String,

    /// Amount to deposit
    #[arg(long)]
    amount: u128,

    /// Principal of the sender (from which account ICRC tokens are taken)
    #[arg(long)]
    sender: Principal,

    /// Principal of the ICRC2 token to be bridged
    #[arg(long)]
    token: Principal,

    /// ERC20 token address
    #[arg(long)]
    erc20_token_address: Address,

    /// IC host
    #[arg(long)]
    ic_host: Option<String>,

    /// Hex-encoded PK to use to sign transaction. If not set, a random wallet will be created.
    #[arg(long)]
    wallet: Option<String>,
}

#[derive(Debug, Parser)]
struct DeployBtfArgs {
    /// ETH address of the bridge
    #[arg(long)]
    minter_address: String,

    /// ETH address of the FeeCharge contract.
    #[arg(long)]
    fee_charge_address: Option<String>,

    /// ETH address of the WrappedTokenDeployer contract.
    #[arg(long)]
    wrapped_token_deployer_address: String,

    /// IsWrappedSide
    #[arg(long, default_value_t = false)]
    is_wrapped_side: bool,

    /// Evm Principal
    #[arg(long)]
    evm: Principal,

    /// IC host
    #[arg(long)]
    ic_host: Option<String>,

    /// Hex-encoded PK to use to sign transaction. If not set, a random wallet will be created.
    #[arg(long)]
    wallet: Option<String>,

    /// Identity Path
    #[arg(long)]
    identity_path: Option<String>,
}

#[derive(Debug, Parser)]
struct DeployWrappedTokenDeployerArgs {
    /// Evm Principal
    #[arg(long)]
    evm: Principal,

    /// IC host
    #[arg(long)]
    ic_host: Option<String>,

    /// Hex-encoded PK to use to sign transaction. If not set, a random wallet will be created.
    #[arg(long)]
    wallet: Option<String>,

    /// Identity Path
    #[arg(long)]
    identity_path: Option<String>,
}

#[derive(Debug, Parser)]
struct DeployFeeChargeArgs {
    /// Principal of the EVM canister
    #[arg(long)]
    evm: Principal,

    #[arg(long)]
    identity_path: Option<String>,

    /// IC host
    #[arg(long)]
    ic_host: Option<String>,

    /// Hex-encoded PK to use to sign transaction.
    #[arg(long)]
    wallet: String,

    /// Nonce for the transaction. Should be fixed to know the contract address before the deploy.
    #[arg(long)]
    nonce: u64,

    /// Addresses of Btfbridges, which should be able to charge fee.
    #[arg(long)]
    bridges: Vec<String>,
}

#[derive(Debug, Parser)]
struct ExpectedContractAddress {
    /// Hex-encoded PK of contract deployer.
    #[arg(long)]
    wallet: String,

    /// Nonce used in contract deployment transaction.
    #[arg(long)]
    nonce: u64,
}

#[derive(Debug, Parser)]
struct CreateTokenArgs {
    /// ETH address of the BTF bridge contract.
    #[arg(long)]
    btf_bridge_address: String,

    /// Name of the token to be created.
    #[arg(long)]
    token_name: String,

    /// Decimal places of the token.
    #[arg(long, default_value = "18")]
    token_decimals: u8,

    /// ID of the source token.
    ///
    /// ID can be in one of the following forms:
    /// * raw hex value of `Id256`
    /// * principal of the token in case the token is hosted by IC
    /// * `BLOCK_ID:TX_INDEX` for runes
    #[arg(long)]
    token_id: String,

    /// Principal of the EVM canister.
    #[arg(long)]
    evm_canister: Principal,

    /// IC host
    #[arg(long)]
    ic_host: Option<String>,

    /// Identity Path
    #[arg(long)]
    identity_path: Option<String>,

    /// Hex-encoded PK to use to sign transaction. If not set, a random wallet will be created.
    #[arg(long)]
    wallet: Option<String>,
}

#[derive(Debug, Parser)]
struct CreateWalletArgs {
    /// Principal of the EVM canister.
    #[arg(long)]
    evm_canister: Principal,
}

#[derive(Debug, Parser)]
struct BurnWrappedArgs {
    /// Hex-encoded PK to use to sign transaction.
    #[arg(long)]
    wallet: String,

    /// Principal of the EVM canister.
    #[arg(long)]
    evm_canister: Principal,

    /// ETH address of the BTF bridge contract.
    #[arg(long)]
    btf_bridge: String,

    /// ETH address of the wrapper token contract.
    #[arg(long)]
    token_address: String,

    /// to Token ID.
    #[arg(long)]
    to_token_id: String,

    /// BTC address to transfer BTC to.
    #[arg(long)]
    address: String,

    /// Amount to transfer.
    #[arg(long)]
    amount: u128,
}

#[derive(Debug, Parser)]
struct WalletAddressArgs {
    /// Hex-encoded PK to use to sign transaction.
    #[arg(long)]
    wallet: String,

    /// If set, returns the address in candid form. Otherwise, in hex form.
    #[arg(long)]
    candid: bool,
}

#[tokio::main]
async fn main() {
    let stdout_logger = tracing_subscriber::fmt::layer()
        .compact()
        .with_ansi(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_writer(std::io::stdout);

    let registry = tracing_subscriber::registry().with(
        stdout_logger
            .with_filter(LevelFilter::TRACE)
            .with_filter(filter::filter_fn(|metadata| {
                metadata.target().starts_with("bridge_tool")
            })),
    );

    tracing::subscriber::set_global_default(registry).expect("failed to set global default");

    match CliCommand::parse() {
        CliCommand::DeployBtfbridge(args) => deploy_btf_bridge(args).await,
        CliCommand::DeployWrappedTokenDeployer(args) => deploy_wrapped_token_deployer(args).await,
        CliCommand::CreateToken(args) => create_token(args).await,
        CliCommand::CreateWallet(args) => create_wallet(args).await,
        CliCommand::BurnWrapped(args) => burn_wrapped(args).await,
        CliCommand::WalletAddress(args) => wallet_address(args),
        CliCommand::DeployFeeCharge(args) => deploy_fee_charge(args).await,
        CliCommand::ExpectedContractAddress(args) => expected_contract_address(args),
        CliCommand::DepositIcrc(args) => deposit_icrc(args).await,
        CliCommand::GetNonce(args) => get_nonce(args).await,
        CliCommand::Deposit(args) => args.run().await.unwrap(),
        CliCommand::Withdraw(args) => args.run().await.unwrap(),
    }
}

async fn get_nonce(args: GetNonceArgs) {
    let host = args.ic_host.as_deref().unwrap_or("http://127.0.0.1:4943");

    let client = EvmCanisterClient::new(
        IcAgentClient::with_identity(args.evm, IDENTITY_PATH, host, None)
            .await
            .expect("Failed to create client"),
    );

    let wallet = get_wallet(&args.wallet, &client).await;
    let nonce = client
        .eth_get_transaction_count(wallet.address().into(), BlockNumber::Pending)
        .await
        .unwrap()
        .unwrap();

    println!("{nonce}");
}

async fn deposit_icrc(args: DepositIcrcArgs) {
    let btf_bridge = Address::from_slice(
        &hex::decode(args.btf_bridge.trim_start_matches("0x"))
            .expect("failed to parse btf bridge address"),
    );

    let host = args.ic_host.as_deref().unwrap_or("http://127.0.0.1:4943");

    let client = EvmCanisterClient::new(
        IcAgentClient::with_identity(args.evm, IDENTITY_PATH, host, None)
            .await
            .expect("Failed to create client"),
    );

    let wallet = get_wallet(&args.wallet, &client).await;
    let chain_id = client.eth_chain_id().await.expect("failed to get chain id");

    let data = Icrc2Burn {
        sender: args.sender,
        amount: args.amount.into(),
        icrc2_token_principal: args.token,
        from_subaccount: None,
        recipient_address: wallet.address().into(),
        approve_after_mint: None,
        fee_payer: None,
        erc20_token_address: args.erc20_token_address.into(),
    };
    let memo = alloy_sol_types::private::FixedBytes::ZERO;

    let input = BTFBridge::notifyMinterCall {
        notificationType: 0,
        userData: Encode!(&data).unwrap().into(),
        memo,
    }
    .abi_encode();

    let nonce = client
        .account_basic(wallet.address().into())
        .await
        .expect("Failed to get account info.")
        .nonce;
    let notify_minter_tx = TransactionBuilder {
        from: &wallet.address().into(),
        to: Some(btf_bridge.into()),
        nonce,
        value: 0u64.into(),
        gas: 5_000_000u64.into(),
        gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
        input,
        signature: SigningMethod::SigningKey(wallet.credential()),
        chain_id,
    }
    .calculate_hash_and_build()
    .expect("failed to sign the transaction");

    let hash = client
        .send_raw_transaction(notify_minter_tx.try_into().expect("failed to convert tx"))
        .await
        .expect("Failed to send raw transaction")
        .expect("Failed to execute deposit notification transaction");

    let receipt = wait_for_tx_success(&client, hash).await;

    //TODO:: Decode the event log

    eprintln!("Deposit notification sent");
    eprintln!("Transaction receipt: {receipt:?}");
}

async fn get_wallet<'a>(
    pk: &'a Option<String>,
    client: &'a EvmCanisterClient<IcAgentClient>,
) -> LocalWallet {
    match pk {
        Some(v) => LocalWallet::from_bytes(
            &(B256::from_slice(
                &hex::decode(v.trim_start_matches("0x")).expect("invalid hex string for wallet PK"),
            )),
        )
        .expect("invalid wallet PK value"),
        None => create_new_wallet(client).await,
    }
}

async fn create_new_wallet(client: &EvmCanisterClient<IcAgentClient>) -> LocalWallet {
    let wallet = LocalWallet::random();
    eprintln!("Initialized new wallet: {:#x}", wallet.address());

    mint_tokens(client, &wallet).await;

    wallet
}

async fn mint_tokens(client: &EvmCanisterClient<IcAgentClient>, wallet: &LocalWallet) {
    let res = client
        .admin_mint_native_tokens(wallet.address().into(), u128::MAX.into())
        .await
        .expect("Failed to send mint native tokens request")
        .expect("Mint native tokens request failed");

    wait_for_tx_success(client, res.0.clone()).await;
    eprintln!(
        "Minted {} ETH tokens to address {:#x}",
        u128::MAX,
        wallet.address()
    );
}

const MAX_TX_TIMEOUT_SEC: u64 = 60;

async fn wait_for_tx_success(
    client: &EvmCanisterClient<IcAgentClient>,
    tx_hash: H256,
) -> TransactionReceipt {
    let start = Instant::now();
    let timeout = Duration::from_secs(MAX_TX_TIMEOUT_SEC);
    while start.elapsed() < timeout {
        let receipt = client
            .eth_get_transaction_receipt(tx_hash.clone())
            .await
            .expect("Failed to request transaction receipt");

        if let Some(receipt) = receipt {
            if receipt.status != Some(1u64.into()) {
                eprintln!("Transaction: {tx_hash}");
                eprintln!("Receipt: {receipt:?}");
                if let Some(output) = receipt.output {
                    let output = String::from_utf8_lossy(&output);
                    eprintln!("Output: {output}");
                }

                panic!("Transaction failed");
            } else {
                return receipt;
            }
        } else {
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    }

    panic!("Transaction {tx_hash} timed out");
}

fn _print_signed_tx(tx: Transaction) {
    let candid_bytes = candid::encode_args((&tx,)).expect("failed to serialize tx to Candid");
    let args = IDLArgs::from_bytes(&candid_bytes).expect("failed to deserialize Candid");
    // Without type annotation instead of field names numerical ids will be used in output
    let args = args
        .annotate_types(false, &TypeEnv::new(), &[Transaction::ty()])
        .unwrap();

    // Output the transaction in Candid text format
    println!("{args}");
}

fn address_from_str(address: &str) -> Address {
    Address::from_slice(
        hex::decode(address.trim_start_matches("0x"))
            .expect("failed to parse address")
            .as_slice(),
    )
}

async fn deploy_btf_bridge(args: DeployBtfArgs) {
    let minter = address_from_str(&args.minter_address);

    let identity = args.identity_path.as_deref().unwrap_or(IDENTITY_PATH);
    let host = args.ic_host.as_deref().unwrap_or("http://127.0.0.1:4943");
    let client = EvmCanisterClient::new(
        IcAgentClient::with_identity(args.evm, identity, host, None)
            .await
            .expect("failed to create evm client"),
    );

    let wallet = get_wallet(&args.wallet, &client).await;

    let chain_id = client.eth_chain_id().await.expect("failed to get chain id");

    let fee_charge = args
        .fee_charge_address
        .map(|address_str| address_from_str(&address_str))
        .unwrap_or_default();

    let wrapped_token_deployer = address_from_str(&args.wrapped_token_deployer_address);

    async fn deploy_contract(
        client: &EvmCanisterClient<IcAgentClient>,
        wallet: &LocalWallet,
        input: Vec<u8>,
        chain_id: u64,
    ) -> Address {
        let nonce = client
            .eth_get_transaction_count(wallet.address().into(), BlockNumber::Pending)
            .await
            .unwrap()
            .unwrap();

        let tx = TransactionBuilder {
            from: &wallet.address().into(),
            to: None,
            nonce,
            value: 0u64.into(),
            gas: 8_000_000u64.into(),
            gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
            input,
            signature: SigningMethod::SigningKey(wallet.credential()),
            chain_id: chain_id as _,
        }
        .calculate_hash_and_build()
        .expect("Failed to sign the transaction");

        let hash = client
            .send_raw_transaction(tx.try_into().expect("failed to convert tx"))
            .await
            .expect("Failed to send raw transaction")
            .expect("Failed to execute contract deployment transaction");
        let receipt = wait_for_tx_success(client, hash).await;
        receipt
            .contract_address
            .expect("Receipt did not contain contract address")
            .into()
    }

    let mut btf_contract_input = BTFBridge::BYTECODE.to_vec();
    let constructor = BTFBridge::constructorCall {}.abi_encode();
    btf_contract_input.extend_from_slice(&constructor);

    let btf_contract_address =
        deploy_contract(&client, &wallet, btf_contract_input, chain_id).await;

    let init_data = BTFBridge::initializeCall {
        minterAddress: minter.0.into(),
        feeChargeAddress: fee_charge.0.into(),
        wrappedTokenDeployer: wrapped_token_deployer.0.into(),
        isWrappedSide: args.is_wrapped_side,
        owner: [0; 20].into(),
        controllers: vec![],
    }
    .abi_encode();

    let mut proxy_input = UUPSProxy::BYTECODE.to_vec();

    let constructor = UUPSProxy::constructorCall {
        _implementation: btf_contract_address.0.into(),
        _data: init_data.into(),
    }
    .abi_encode();
    proxy_input.extend_from_slice(&constructor);

    let btf_proxy_address = deploy_contract(&client, &wallet, proxy_input, chain_id).await;

    eprintln!("Created BTF Bridge contract");
    println!("Implementation address: {btf_contract_address:#x}");
    println!("Proxy address: {btf_proxy_address:#x}");
    println!("{btf_proxy_address:#x}");
}

async fn deploy_wrapped_token_deployer(args: DeployWrappedTokenDeployerArgs) {
    let identity = args.identity_path.as_deref().unwrap_or(IDENTITY_PATH);
    let host = args.ic_host.as_deref().unwrap_or("http://127.0.0.1:4943");
    let client = EvmCanisterClient::new(
        IcAgentClient::with_identity(args.evm, identity, host, None)
            .await
            .expect("failed to create evm client"),
    );

    let some_wallet = args.wallet.clone();
    let wallet = get_wallet(&some_wallet, &client).await;
    let did_from: did::H160 = wallet.address().into();

    let deploy_tx_input = WrappedTokenDeployer::BYTECODE.to_vec();

    let chain_id = client.eth_chain_id().await.expect("failed to get chain id");
    let nonce = client
        .account_basic(did_from.clone())
        .await
        .expect("failed to basic account")
        .nonce;

    let create_contract_tx = TransactionBuilder {
        from: &did_from,
        input: deploy_tx_input,
        nonce,
        gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
        to: None,
        value: U256::zero(),
        gas: 4_000_000_u64.into(),
        signature: SigningMethod::SigningKey(wallet.credential()),
        chain_id,
    }
    .calculate_hash_and_build()
    .expect("failed to build transaction");

    let hash = client
        .send_raw_transaction(create_contract_tx.try_into().expect("failed to convert tx"))
        .await
        .expect("Failed to send raw transaction")
        .expect("Failed to execute crate BTF contract transaction");
    let receipt = wait_for_tx_success(&client, hash).await;
    let wrapped_token_deployer_contract_address = receipt
        .contract_address
        .expect("Receipt did not contain contract address");

    eprintln!("Created WrappedTokenDeployer contract");
    println!("{wrapped_token_deployer_contract_address:#x}");
}

async fn deploy_fee_charge(args: DeployFeeChargeArgs) {
    let identity = args.identity_path.as_deref().unwrap_or(IDENTITY_PATH);
    let host = args.ic_host.as_deref().unwrap_or("http://127.0.0.1:4943");
    let client = EvmCanisterClient::new(
        IcAgentClient::with_identity(args.evm, identity, host, None)
            .await
            .expect("failed to create evm client"),
    );

    let some_wallet = Some(args.wallet.clone());
    let wallet = get_wallet(&some_wallet, &client).await;
    let did_from: did::H160 = wallet.address().into();

    let addresses = args
        .bridges
        .iter()
        .map(|addr| {
            let addr = Address::from_slice(
                &hex::decode(addr.trim_start_matches("0x"))
                    .expect("failed to parse bridge address"),
            );
            addr.0.into()
        })
        .collect();
    let mut fee_charge_input = FeeCharge::BYTECODE.to_vec();
    let constructor = FeeCharge::constructorCall {
        canChargeFee: addresses,
    }
    .abi_encode();

    fee_charge_input.extend_from_slice(&constructor);

    let chain_id = client.eth_chain_id().await.expect("failed to get chain id");

    let create_contract_tx = TransactionBuilder {
        from: &did_from,
        input: fee_charge_input,
        nonce: args.nonce.into(),
        gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
        to: None,
        value: U256::zero(),
        gas: 4_000_000_u64.into(),
        signature: SigningMethod::SigningKey(wallet.credential()),
        chain_id,
    }
    .calculate_hash_and_build()
    .expect("failed to build transaction");

    let hash = client
        .send_raw_transaction(create_contract_tx.try_into().expect("failed to convert tx"))
        .await
        .expect("Failed to send raw transaction")
        .expect("Failed to execute crate BTF contract transaction");
    let receipt = wait_for_tx_success(&client, hash).await;
    let fee_charge_contract_address = receipt
        .contract_address
        .expect("Receipt did not contain contract address");

    eprintln!("Created FeeCharge contract");
    println!("{fee_charge_contract_address:#x}");
}

fn expected_contract_address(args: ExpectedContractAddress) {
    let wallet = LocalWallet::from_bytes(
        &(B256::from_slice(
            &hex::decode(args.wallet.trim_start_matches("0x"))
                .expect("invalid hex string for wallet PK"),
        )),
    )
    .expect("invalid wallet PK value");
    let deployer = wallet.address();
    let contract_address = bridge_utils::get_contract_address(deployer, U256::from(args.nonce));
    println!("{contract_address:#x}");
}

async fn create_token(args: CreateTokenArgs) {
    let btf_bridge = address_from_str(&args.btf_bridge_address);

    let token_id = decode_token_id(&args.token_id)
        .unwrap_or_else(|| panic!("Invalid token id format: {}", args.token_id));

    let identity = args.identity_path.as_deref().unwrap_or(IDENTITY_PATH);
    let host = args.ic_host.as_deref().unwrap_or("http://127.0.0.1:4943");

    let client = EvmCanisterClient::new(
        IcAgentClient::with_identity(args.evm_canister, identity, host, None)
            .await
            .expect("Failed to create client"),
    );

    let wallet = get_wallet(&args.wallet, &client).await;
    let chain_id = client.eth_chain_id().await.expect("failed to get chain id");

    let input = BTFBridge::deployERC20Call {
        name: args.token_name.clone(),
        symbol: args.token_name,
        decimals: args.token_decimals,
        baseTokenID: token_id.0.into(),
    }
    .abi_encode();

    let nonce = client
        .account_basic(wallet.address().into())
        .await
        .expect("Failed to get account info.")
        .nonce;
    let create_token_tx = TransactionBuilder {
        from: &wallet.address().into(),
        to: Some(btf_bridge.into()),
        nonce,
        value: 0u64.into(),
        gas: 5_000_000u64.into(),
        gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
        input,
        signature: SigningMethod::SigningKey(wallet.credential()),
        chain_id,
    }
    .calculate_hash_and_build()
    .expect("failed to sign the transaction");

    let hash = client
        .send_raw_transaction(create_token_tx.try_into().expect("failed to convert tx"))
        .await
        .expect("Failed to send raw transaction")
        .expect("Failed to execute crate token transaction");
    let receipt = wait_for_tx_success(&client, hash).await;

    let token_address = BTFBridge::deployERC20Call::abi_decode_returns(
        &receipt
            .output
            .expect("Receipt for token creation does not contain output"),
        true,
    )
    .expect("Failed to decode token creation output")
    ._0;

    eprintln!("Created token contract");
    println!("{:#x}", token_address);
}

async fn create_wallet(args: CreateWalletArgs) {
    let client = EvmCanisterClient::new(
        IcAgentClient::with_identity(
            args.evm_canister,
            IDENTITY_PATH,
            "http://127.0.0.1:4943",
            None,
        )
        .await
        .expect("Failed to create client"),
    );

    let wallet = create_new_wallet(&client).await;

    eprint!("Wallet address, Candid style: blob \"");
    for num in wallet.address().0 {
        eprint!("\\{num:02x}");
    }
    for _ in 0..12 {
        eprint!("\\00");
    }
    eprintln!("\"");

    println!("0x{}", hex::encode(wallet.credential().to_bytes()));
}

fn wallet_address(args: WalletAddressArgs) {
    let wallet_pk = hex::decode(args.wallet.trim_start_matches("0x"))
        .expect("Failed to decode wallet pk from hex string");
    let wallet = LocalWallet::from_bytes(&(B256::from_slice(&wallet_pk)))
        .expect("Failed to create a wallet");

    if args.candid {
        print!("blob \"");
        for num in wallet.address().0 {
            print!("\\{num:02x}");
        }
        for _ in 0..12 {
            print!("\\00");
        }
        println!("\"");
    } else {
        println!("{:#x}", wallet.address());
    }
}

async fn burn_wrapped(args: BurnWrappedArgs) {
    let client = EvmCanisterClient::new(
        IcAgentClient::with_identity(
            args.evm_canister,
            IDENTITY_PATH,
            "http://127.0.0.1:4943",
            None,
        )
        .await
        .expect("Failed to create client"),
    );

    let wallet_addr = Some(args.wallet.clone());
    let wallet = get_wallet(&wallet_addr, &client).await;
    let chain_id = client.eth_chain_id().await.expect("failed to get chain id");

    let btf_bridge = address_from_str(&args.btf_bridge);
    let token = address_from_str(&args.token_address);

    let input = WrappedToken::balanceOfCall {
        account: wallet.address().0.into(),
    }
    .abi_encode();

    let result = client
        .eth_call(
            Some(wallet.address().into()),
            Some(token.into()),
            None,
            5_000_000u64,
            Some((EIP1559_INITIAL_BASE_FEE * 2).into()),
            Some(input.into()),
        )
        .await
        .expect("balance call failed")
        .expect("balance call failed");
    let balance = u128::from_str_radix(result.trim_start_matches("0x"), 16)
        .expect("Failed to decode balance response");
    eprintln!("Current wrapped token balance: {balance}");

    let amount: U256 = args.amount.into();

    let input = WrappedToken::approveCall {
        spender: btf_bridge.0.into(),
        value: amount.clone().into(),
    }
    .abi_encode();
    let nonce = client
        .account_basic(wallet.address().into())
        .await
        .expect("Failed to get account info.")
        .nonce;
    let approve_tx = TransactionBuilder {
        from: &wallet.address().into(),
        to: Some(token.into()),
        nonce,
        value: 0u64.into(),
        gas: 5_000_000u64.into(),
        gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
        input,
        signature: SigningMethod::SigningKey(wallet.credential()),
        chain_id,
    }
    .calculate_hash_and_build()
    .expect("failed to sign the transaction");

    let hash = client
        .send_raw_transaction(approve_tx.try_into().expect("failed to convert tx"))
        .await
        .expect("Failed to send raw transaction")
        .expect("Failed to execute approve transaction");
    wait_for_tx_success(&client, hash).await;

    let memo = alloy_sol_types::private::FixedBytes::ZERO;

    let input = BTFBridge::burnCall {
        amount: amount.into(),
        fromERC20: token.0.into(),
        toTokenID: alloy_sol_types::private::FixedBytes::from_slice(args.to_token_id.as_bytes()),
        recipientID: args.address.into_bytes().into(),
        memo,
    }
    .abi_encode();

    let nonce = client
        .account_basic(wallet.address().into())
        .await
        .expect("Failed to get account info.")
        .nonce;
    let burn_tx = TransactionBuilder {
        from: &wallet.address().into(),
        to: Some(btf_bridge.into()),
        nonce,
        value: 0u64.into(),
        gas: 5_000_000u64.into(),
        gas_price: (EIP1559_INITIAL_BASE_FEE * 2).into(),
        input,
        signature: SigningMethod::SigningKey(wallet.credential()),
        chain_id,
    }
    .calculate_hash_and_build()
    .expect("failed to sign the transaction");

    let hash = client
        .send_raw_transaction(burn_tx.try_into().expect("failed to convert tx"))
        .await
        .expect("Failed to send raw transaction")
        .expect("Failed to execute burn transaction");
    wait_for_tx_success(&client, hash).await;
}

fn decode_token_id(id_string: &str) -> Option<Id256> {
    if let Ok(hex) = hex::decode(id_string) {
        if hex.len() == 32 {
            return Id256::from_slice(&hex);
        }
    }

    let split: Vec<_> = id_string.split(':').collect();
    if split.len() == 2 {
        let block_id = split[0]
            .parse::<u64>()
            .unwrap_or_else(|_| panic!("invalid rune id: {id_string})"));
        let tx_index = split[1]
            .parse::<u32>()
            .unwrap_or_else(|_| panic!("invalid rune id: {id_string})"));
        return Some(Id256::from_btc_tx_index(block_id, tx_index));
    }

    if let Ok(id) = Principal::from_str(id_string) {
        return Some(Id256::from(&id));
    }

    None
}
