use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::Error;
use bitfinity_benchmark::bridge_user::{BridgeUser, UserStats, ICRC1_TRANSFER_FEE};
use candid::{Nat, Principal};
use clap::Parser;
use ethers_core::k256::elliptic_curve::SecretKey;
use evm_canister_client::ic_agent::identity::Secp256k1Identity;
use evm_canister_client::ic_agent::{Agent, Identity};
use ic_exports::icrc_types::icrc::generic_metadata_value::MetadataValue;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_exports::icrc_types::icrc1_ledger::{
    ArchiveOptions, FeatureFlags, InitArgs, LedgerArgument,
};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// Simple CLI program for Benchmarking BitFinity Network
#[derive(Parser, Debug)]
#[clap(version = "0.1", about = "Tool for benchmarking MinterCanister")]
struct BenchmarkArgs {
    /// Test time in seconds
    #[arg(long, short('t'), default_value = "60")]
    test_time: u32,

    /// The number of users to simulate.
    #[arg(long, short('u'), default_value = "10")]
    users_number: u32,

    /// Minter cansiter principal.
    /// A result of `dfx canister id minter` will be used as default.
    #[arg(long = "mc")]
    minter_canister_principal: Option<String>,

    /// EVM cansiter principal.
    /// A result of `dfx canister id evm` will be used as default.
    #[arg(long = "evmc")]
    evm_canister_principal: Option<String>,

    /// IC network to use.
    #[arg(long, short('n'), default_value = "local")]
    network: String,

    /// URL address of the replica to communicate.
    #[arg(long, short('r'))]
    replica_url: Option<String>,

    /// Name of admin's identity, which will be used to deploy ICRC-2 tokens.
    #[arg(long, default_value = "bridge_admin")]
    admin_identity_name: String,

    /// If this argument is set, the tool will remove dfx identities with names `bridge_user_{i}`
    /// where `i in 0..given_number`.
    #[arg(long)]
    remove_users: Option<u32>,

    /// Number of Base ICRC-2 tokens to deploy and use for bridge operations.
    #[arg(long, default_value = "4")]
    icrc2_tokens_number: u32,

    /// Path to the ICRC-2 tokens wasm binary.
    #[arg(long, default_value = "./.artifact/icrc1-ledger.wasm.gz")]
    icrc2_wasm_file: PathBuf,
}

impl BenchmarkArgs {
    pub const ICRC_2_TOKEN_SYMBOL_PREFIX: &'static str = "BTKN";
    pub const USER_IDENTITY_NAME_PREFIX: &'static str = "bridge_user_";

    /// Get EVMc principal if present in args, or from `dfx canister id evm`.
    pub fn get_evm_canister_principal(&self) -> anyhow::Result<Principal> {
        let text = self
            .evm_canister_principal
            .clone()
            .ok_or_else(|| Error::msg("No evm canister principal provided in args"))
            .or_else(|_| self.dfx_canister_id("evm"))?;

        Ok(Principal::from_text(text)?)
    }

    /// Get minter canister principal if present in args, or from `dfx canister id minter`.
    pub fn get_minter_canister_principal(&self) -> anyhow::Result<Principal> {
        let text = self
            .evm_canister_principal
            .clone()
            .ok_or_else(|| Error::msg("No minter canister principal provided in args"))
            .or_else(|_| self.dfx_canister_id("minter"))?;

        Ok(Principal::from_text(text)?)
    }

    /// Returns result of `dfx canister id {canister_name}`.
    pub fn dfx_canister_id(&self, canister_name: &str) -> anyhow::Result<String> {
        let output = Command::new("dfx")
            .args(["canister", "id", canister_name, "--network", &self.network])
            .output()?;

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }

    /// Get or create dfx identity.
    pub fn get_or_create_dfx_identity(&self, user_name: &str) -> anyhow::Result<impl Identity> {
        if let Err(e) = self.create_dfx_identity(user_name) {
            log::info!("Failed to create identity {user_name}: {e}");
        }

        self.get_dfx_identity(user_name)
    }

    /// Create dfx identity.
    pub fn create_dfx_identity(&self, user_name: &str) -> anyhow::Result<()> {
        let output = Command::new("dfx")
            .args(["identity", "new", user_name, "--network", &self.network])
            .output()?;

        if output.status.success() {
            log::info!(
                "Created identity `{user_name}`: {:?}",
                String::from_utf8(output.stdout)
            );
        }

        Ok(())
    }

    /// Get dfx identity.
    pub fn get_dfx_identity(&self, user_name: &str) -> anyhow::Result<impl Identity> {
        let output = Command::new("dfx")
            .args(["identity", "export", user_name, "--network", &self.network])
            .output()?;

        let pem_str = String::from_utf8(output.stdout)?;

        Ok(Secp256k1Identity::from_private_key(
            SecretKey::from_sec1_pem(&pem_str)?,
        ))
    }

    /// Get dfx replica port.
    pub fn get_dfx_replica_port(&self) -> anyhow::Result<String> {
        let output = Command::new("dfx")
            .args(["info", "replica-port"])
            .output()?;

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }

    /// Get replica URL from args or from `dfx info replica-port`.
    pub fn get_replica_url(&self) -> anyhow::Result<String> {
        if let Some(url) = &self.replica_url {
            return Ok(url.clone());
        }

        let dfx_replica_port = self.get_dfx_replica_port()?;
        Ok(format!("http://localhost:{dfx_replica_port}"))
    }

    async fn agent_by_name(&self, identity_name: &str) -> anyhow::Result<Agent> {
        let identity = self.get_or_create_dfx_identity(identity_name)?;
        let agent = Agent::builder()
            .with_identity(identity)
            .with_url(self.get_replica_url()?)
            .build()?;
        agent.fetch_root_key().await?;
        Ok(agent)
    }

    fn user_name_by_idx(idx: u32) -> String {
        format!("{}{idx}", Self::USER_IDENTITY_NAME_PREFIX)
    }

    fn token_symbol_by_idx(idx: u32) -> String {
        format!("{}{idx}", Self::ICRC_2_TOKEN_SYMBOL_PREFIX)
    }

    fn token_name_by_idx(idx: u32) -> String {
        format!("{}{idx} token", Self::ICRC_2_TOKEN_SYMBOL_PREFIX)
    }

    async fn load_icrc2_token_wasm(&self) -> anyhow::Result<Vec<u8>> {
        load_wasm_bytecode(&self.icrc2_wasm_file).await
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = BenchmarkArgs::parse();

    if let Some(users_to_remove) = args.remove_users {
        remove_users(users_to_remove)?;
        return Ok(());
    }

    let admin_agent = args.agent_by_name(&args.admin_identity_name).await?;

    log::trace!("Creating icrc2 token canisters");
    let icrc2_tokens = create_icrc2_tokens(&args, &admin_agent).await?;

    let admin = create_bridge_user(&args, args.admin_identity_name.clone(), admin_agent.clone())?;
    admin
        .mint_bft_native_tokens(10u64.pow(18u32).into())
        .await?;

    admin.add_operation_points().await?;

    log::trace!("Creating wrapped tokens");
    create_wrapped_tokens(&admin, &icrc2_tokens).await?;

    let users = create_users(&args).await?;

    log::trace!("Minting native tokens for users");
    let mint_native_tokens_futures = users
        .iter()
        .map(|user| user.mint_bft_native_tokens(10u64.pow(18u32).into()));
    let mint_native_tokens_results = futures::future::join_all(mint_native_tokens_futures).await;
    mint_native_tokens_results
        .into_iter()
        .try_for_each(|result| result.map(|_| ()))?;

    log::trace!("Distributing base tokens for users");
    distribute_base_tokens_to_users(&admin, &users, &icrc2_tokens).await?;

    log::trace!("Simulating users actions");
    let test_duration = Duration::from_secs(args.test_time as _);
    let statistics_futures_iter = users
        .into_iter()
        .map(move |user| user.run(icrc2_tokens.clone(), test_duration));

    let statistics = futures::future::join_all(statistics_futures_iter).await;

    let mut total_statistic = UserStats::default();
    for (idx, statistic) in statistics.iter().enumerate() {
        log::info!("Statistic for user {idx}: {:?}", statistic);
        total_statistic.merge(statistic);
    }

    log::info!("Total statistic: {:?}", total_statistic);

    Ok(())
}

async fn distribute_base_tokens_to_users(
    admin: &BridgeUser,
    users: &[BridgeUser],
    icrc2_tokens: &[Principal],
) -> anyhow::Result<()> {
    let total_fee = ICRC1_TRANSFER_FEE * users.len() as u64;
    let amount = (INIT_ICRC1_BALANCE - total_fee) / users.len() as u64;

    for user in users {
        let trensfer_futures_iter = icrc2_tokens
            .iter()
            .map(|token| admin.finish_icrc2_mint(*token, None, user, amount.into()));

        let results = futures::future::join_all(trensfer_futures_iter).await;
        results
            .into_iter()
            .try_for_each(|result| result.map(|_| ()))?;
    }

    Ok(())
}

async fn create_wrapped_tokens(
    admin: &BridgeUser,
    base_tokens: &[Principal],
) -> anyhow::Result<()> {
    for (idx, token) in base_tokens.iter().enumerate() {
        let symbol = BenchmarkArgs::token_symbol_by_idx(idx as _);
        admin.create_wrapped_token(*token, symbol).await?;
    }

    Ok(())
}

async fn create_users(args: &BenchmarkArgs) -> anyhow::Result<Vec<BridgeUser>> {
    let mut users = Vec::with_capacity(args.users_number as _);
    for i in 0..args.users_number {
        let identity_name = BenchmarkArgs::user_name_by_idx(i);
        let agent = args.agent_by_name(&identity_name).await?;
        let user = create_bridge_user(args, identity_name, agent)?;
        users.push(user);
    }

    Ok(users)
}

fn create_bridge_user(
    args: &BenchmarkArgs,
    name: String,
    agent: Agent,
) -> anyhow::Result<BridgeUser> {
    let evm_principal = args.get_evm_canister_principal()?;
    let minter_canister_principal = args.get_minter_canister_principal()?;

    BridgeUser::new(name, agent, evm_principal, minter_canister_principal)
}

fn remove_users(users_number: u32) -> anyhow::Result<()> {
    for i in 0..users_number {
        let user_name = BenchmarkArgs::user_name_by_idx(i);
        let output = Command::new("dfx")
            .args(["identity", "remove", &user_name])
            .output();

        let output = match output {
            Ok(out) => out,
            Err(e) => {
                log::warn!("Failed to remove identity `{user_name}`: {e}");
                continue;
            }
        };

        if !output.status.success() {
            log::warn!(
                "Failed to remove identity `{user_name}`: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            continue;
        }

        log::info!("Removed identity `{user_name}`");
    }

    Ok(())
}

async fn create_icrc2_tokens(
    args: &BenchmarkArgs,
    admin: &Agent,
) -> anyhow::Result<Vec<Principal>> {
    let mut token_principals = Vec::with_capacity(args.icrc2_tokens_number as _);
    let wasm = args.load_icrc2_token_wasm().await?;
    for i in 0..args.icrc2_tokens_number {
        let token_symbol = BenchmarkArgs::token_symbol_by_idx(i);
        let token_name = BenchmarkArgs::token_name_by_idx(i);
        let principal = create_icrc2_token_canister(
            admin,
            &args.admin_identity_name,
            token_name,
            token_symbol,
            &wasm,
        )
        .await?;
        log::info!("ICRC-2 token canister created: {principal}");
        token_principals.push(principal);
    }

    Ok(token_principals)
}

pub const INIT_ICRC1_BALANCE: u64 = 10_u64.pow(18);

pub async fn create_icrc2_token_canister(
    admin: &Agent,
    admin_identity_name: &str,
    token_name: String,
    token_symbol: String,
    wasm: &[u8],
) -> anyhow::Result<Principal> {
    log::trace!("Creating ICRC1 token canister '{token_name}'");

    let admin_principal = admin
        .get_principal()
        .expect("Admin principal should be available");

    let init_args = InitArgs {
        minting_account: Account::from(admin_principal),
        fee_collector_account: None,
        initial_balances: vec![(
            Account::from(admin_principal),
            Nat::from(INIT_ICRC1_BALANCE),
        )],
        transfer_fee: Nat::from(ICRC1_TRANSFER_FEE),
        token_name: token_name.clone(),
        token_symbol: token_symbol.clone(),
        metadata: vec![(
            "icrc1:name".to_string(),
            MetadataValue::Text("Tokenium".to_string()),
        )],
        archive_options: ArchiveOptions {
            trigger_threshold: 10,
            num_blocks_to_archive: 5,
            node_max_memory_size_bytes: None,
            max_message_size_bytes: None,
            controller_id: admin_principal,
            cycles_for_archive_creation: None,
            max_transactions_per_response: None,
        },
        max_memo_length: None,
        feature_flags: Some(FeatureFlags { icrc2: true }),
        decimals: None,
        maximum_number_of_accounts: None,
        accounts_overflow_trim_quantity: None,
    };

    let args = LedgerArgument::Init(init_args);

    let token = ic_test_utils::create_canister(
        admin,
        admin_identity_name,
        wasm.into(),
        (args, Nat::from(100_000_000_000u64)),
        200_000_000_000,
    )
    .await
    .unwrap();

    Ok(token)
}

async fn load_wasm_bytecode(path: &Path) -> anyhow::Result<Vec<u8>> {
    log::trace!("Loading wasm bytecode from {path:?}");

    let mut f = File::open(path).await?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer).await?;

    Ok(buffer)
}
