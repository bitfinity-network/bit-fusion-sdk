use std::path::PathBuf;

use clap::{ArgAction, Parser};
use ethereum_types::H256;
use tracing::level_filters::LevelFilter;
use tracing::{debug, info, trace, Level};

use crate::commands::{BFTArgs, Commands};
use crate::contracts::EvmNetwork;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The main CLI struct for the Bitfinity Deployer.
#[derive(Parser, Debug)]
#[command(author, version = VERSION, about = "Bitfinity Deployer", long_about = None)]
pub struct Cli {
    /// The command to run
    #[command(subcommand)]
    command: Commands,
    /// The identity that will be used to perform the DFX operations
    #[arg(long, value_name = "IDENTITY_PATH")]
    identity: PathBuf,

    /// Private Key of the wallet to use for the transaction
    #[arg(short('p'), long, value_name = "PRIVATE_KEY", env)]
    private_key: H256,

    /// Ths is the host of the IC.
    #[arg(
        short,
        long,
        value_name = "IC_HOST",
        default_value = "http://localhost:8080",
        help_heading = "IC Host"
    )]
    ic_host: String,

    /// Deploy the BFT bridge.
    #[arg(long, default_value = "false", help_heading = "Bridge Contract Args")]
    deploy_bft: bool,

    /// These are extra arguments for the BFT bridge.
    #[command(flatten, next_help_heading = "Bridge Contract Args")]
    bft_args: BFTArgs,

    /// EVM network to deploy the contract to (e.g. "mainnet", "testnet", "local")
    #[arg(
        long,
        value_name = "EVM_NETWORK",
        default_value = "local",
        help_heading = "Bridge Contract Args"
    )]
    evm_network: EvmNetwork,

    /// Set the minimum log level.
    ///
    /// -v      Errors
    /// -vv     Warnings
    /// -vvv    Info
    /// -vvvv   Debug
    /// -vvvvv  Traces (warning: very verbose!)
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
            ic_host,
            deploy_bft,
            bft_args,
            evm_network,
            command,
            ..
        } = cli;

        info!("Starting Bitfinity Deployer v{}", VERSION);
        debug!("IC host: {}", ic_host);

        trace!("Executing command: {:?}", command);
        command
            .run(
                identity.to_path_buf(),
                &ic_host,
                evm_network,
                private_key,
                deploy_bft,
                bft_args,
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
                3 => Level::DEBUG,
                _ => Level::TRACE,
            };

            level.into()
        }
    }

    /// Initializes tracing with the appropriate log level based on the verbosity setting.
    pub fn init_tracing(&self) {
        let directive = self.level();
        tracing_subscriber::fmt()
            .with_max_level(directive)
            .with_target(true)
            .with_ansi(true)
            .init();
    }
}
