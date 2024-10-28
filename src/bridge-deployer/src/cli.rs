use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::bail;
use candid::Principal;
use clap::{ArgAction, Parser};
use ethereum_types::H256;
use ic_agent::identity::{BasicIdentity, Secp256k1Identity};
use ic_agent::Identity;
use ic_canister_client::agent::identity::GenericIdentity;
use tracing::level_filters::LevelFilter;
use tracing::{debug, info, trace, Level};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::{filter, Layer as _};

use crate::canister_ids::CanisterIdsPath;
use crate::commands::Commands;
use crate::contracts::EvmNetwork;
use crate::evm::evm_principal_or_default;

/// The main CLI struct for the Bitfinity Deployer.
#[derive(Parser, Debug)]
#[command(author, version, about = "Bitfinity Deployer", long_about = None)]
pub struct Cli {
    /// The command to run
    #[command(subcommand)]
    command: Commands,

    /// The identity that will be used to perform the DFX operations
    ///
    /// If not set, current active DFX identity will be used
    #[arg(long, value_name = "IDENTITY_PATH")]
    identity: Option<PathBuf>,

    /// Private Key of the wallet to use for the transaction
    ///
    /// This must be provided in all the commands except for the `upgrade` command.
    #[arg(short('p'), long, value_name = "PRIVATE_KEY", env)]
    private_key: H256,

    /// EVM network to deploy the contract to
    #[arg(
        long,
        value_name = "EVM_NETWORK",
        default_value = "localhost",
        help_heading = "Bridge Contract Args"
    )]
    evm_network: EvmNetwork,

    /// Optional EVM canister to link to; if not provided, the default one will be used based on the network
    #[arg(long)]
    pub evm: Option<Principal>,

    /// Set the minimum log level.
    ///
    /// -v      Errors
    /// -vv     Warnings
    /// -vvv    Info
    /// -vvvv   Debug
    /// -vvvvv  Debug with other libraries
    /// -vvvvvv  Traces (warning: very verbose!)
    #[arg(short, long, action = ArgAction::Count, global = true, default_value_t = 3, verbatim_doc_comment, help_heading = "Display")]
    verbosity: u8,

    #[arg(
        long,
        alias = "silent",
        short = 'q',
        global = true,
        help_heading = "Display"
    )]
    quiet: bool,

    /// Custom path to the canister_ids.json file.
    ///
    /// If not provided, the default path for the provided evm network is used.
    #[arg(
        long,
        value_name = "CANISTER_IDS_PATH",
        help_heading = "Path to Canister IDs"
    )]
    canister_ids: Option<PathBuf>,
}

impl Cli {
    /// Runs the Bitfinity Deployer application.
    pub async fn run() -> anyhow::Result<()> {
        let _ = dotenv::dotenv();
        let cli = Cli::parse();

        // Initialize tracing with the appropriate log level based on the verbosity setting.
        cli.init_tracing();
        let identity = cli.init_identity()?;
        info!(
            "Using dfx identity with principal: {}",
            identity.sender().expect("invalid agent identity"),
        );

        let Cli {
            private_key,
            evm,
            evm_network,
            command,
            canister_ids,
            ..
        } = cli;

        // derive arguments
        let ic_host = crate::evm::ic_host(evm_network);

        println!("Starting Bitfinity Deployer v{}", env!("CARGO_PKG_VERSION"));
        debug!("IC host: {}", ic_host);

        // load canister ids file
        let canister_ids_path = canister_ids
            .map(|path| CanisterIdsPath::CustomPath(path, evm_network))
            .unwrap_or_else(|| CanisterIdsPath::from(evm_network));

        debug!("Canister ids path: {}", canister_ids_path.path().display());

        trace!("Executing command: {:?}", command);
        command
            .run(
                identity,
                &ic_host,
                evm_network,
                evm_principal_or_default(evm_network, evm),
                private_key,
                canister_ids_path,
            )
            .await?;

        Ok(())
    }
    /// Get the corresponding [LevelFilter] for the given verbosity, or none if the verbosity
    /// corresponds to silent.
    pub fn level(&self) -> LevelFilter {
        if self.quiet {
            LevelFilter::OFF
        } else {
            let level = match self.verbosity - 1 {
                0 => Level::ERROR,
                1 => Level::WARN,
                2 => Level::INFO,
                3 | 4 => Level::DEBUG,
                _ => Level::TRACE,
            };

            level.into()
        }
    }

    /// Initializes tracing with the appropriate log level based on the verbosity setting.
    pub fn init_tracing(&self) {
        let stdout_logger = tracing_subscriber::fmt::layer()
            .compact()
            .with_ansi(true)
            .with_span_events(FmtSpan::CLOSE)
            .with_writer(std::io::stdout);

        let registry = tracing_subscriber::registry().with(
            stdout_logger
                .with_filter(self.level())
                .with_filter(filter::filter_fn(self.source_filter())),
        );

        tracing::subscriber::set_global_default(registry).expect("failed to set global default");
    }

    /// Returns a filter function that filters out log messages based on the verbosity level.
    fn source_filter(&self) -> impl Fn(&tracing::Metadata<'_>) -> bool {
        if self.verbosity - 1 > 3 {
            Self::filter_none
        } else {
            Self::filter_deployer_only
        }
    }

    #[inline]
    /// Filters out log messages that are not from the deployer.
    fn filter_deployer_only(metadata: &tracing::Metadata) -> bool {
        metadata.target().starts_with("bridge_deployer")
    }

    #[inline]
    /// Filters out no log messages.
    fn filter_none(_metadata: &tracing::Metadata) -> bool {
        true
    }

    /// Returns DFX identity to be used.
    ///
    /// If configured though CLI, returns the set one. Otherwise, gets the currently active identity
    /// from the DFX.
    fn init_identity(&self) -> anyhow::Result<GenericIdentity> {
        if let Some(path) = &self.identity {
            Ok(GenericIdentity::try_from(path.as_ref())?)
        } else {
            let result = Command::new("dfx")
                .args(vec!["identity", "whoami"])
                .stdout(Stdio::piped())
                .output()?;
            if !result.status.success() {
                bail!(
                    "Failed to get dfx identity name: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
            }

            let identity_name = String::from_utf8(result.stdout)?;
            let identity_name = identity_name.trim();

            let result = Command::new("dfx")
                .args(vec!["identity", "export", &identity_name])
                .stdout(Stdio::piped())
                .output()?;

            if !result.status.success() {
                bail!(
                    "Failed to get dfx identity PEM: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
            }

            Ok(Secp256k1Identity::from_pem(&result.stdout[..])
                .map(GenericIdentity::from)
                .or(BasicIdentity::from_pem(&result.stdout[..]).map(GenericIdentity::from))?)
        }
    }
}
