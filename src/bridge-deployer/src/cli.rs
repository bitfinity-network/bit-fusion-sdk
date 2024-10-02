use std::path::PathBuf;

use clap::{ArgAction, Parser};
use ethereum_types::H256;
use tracing::level_filters::LevelFilter;
use tracing::{debug, trace, Level};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::{filter, Layer as _};

use crate::commands::Commands;
use crate::contracts::EvmNetwork;

/// The main CLI struct for the Bitfinity Deployer.
#[derive(Parser, Debug)]
#[command(author, version, about = "Bitfinity Deployer", long_about = None)]
pub struct Cli {
    /// The command to run
    #[command(subcommand)]
    command: Commands,
    /// The identity that will be used to perform the DFX operations
    #[arg(long, value_name = "IDENTITY_PATH")]
    identity: PathBuf,

    /// Private Key of the wallet to use for the transaction
    ///
    /// This must be provided in all the commands except for the `upgrade` command.
    #[arg(short('p'), long, value_name = "PRIVATE_KEY", env)]
    private_key: H256,

    /// EVM network to deploy the contract to (e.g. "mainnet", "testnet", "local")
    #[arg(
        long,
        value_name = "EVM_NETWORK",
        default_value = "localhost",
        help_heading = "Bridge Contract Args"
    )]
    evm_network: EvmNetwork,

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
}

impl Cli {
    /// Runs the Bitfinity Deployer application.
    pub async fn run() -> anyhow::Result<()> {
        let cli = Cli::parse();

        // Initialize tracing with the appropriate log level based on the verbosity setting.
        cli.init_tracing();

        let Cli {
            identity,
            private_key,
            evm_network,
            command,
            ..
        } = cli;

        // derive arguments
        let ic_host = crate::evm::ic_host(evm_network);

        println!("Starting Bitfinity Deployer v{}", env!("CARGO_PKG_VERSION"));
        debug!("IC host: {}", ic_host);

        trace!("Executing command: {:?}", command);
        command
            .run(identity.to_path_buf(), &ic_host, evm_network, private_key)
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
}
