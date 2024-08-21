use std::ffi::OsString;
use std::path::PathBuf;

use clap::{ArgAction, Parser};
use tracing::level_filters::LevelFilter;
use tracing::{debug, info, trace, Level};
use tracing_subscriber::filter::Directive;
use tracing_subscriber::EnvFilter;

use crate::canister_manager::CanisterManager;
use crate::commands::Commands;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Some operations with BFT bridge.
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
    #[arg(short('p'), long, value_name = "PRIVATE_KEY")]
    private_key: String,

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

    /// Ths is the host of the IC.
    #[arg(
        short,
        long,
        value_name = "IC_HOST",
        default_value = "http://localhost:8080",
        help_heading = "IC Host"
    )]
    ic_host: String,

    /// Path to the canister manager state file
    #[arg(long, value_name = "STATE_FILE", default_value = "canister_state.json")]
    state_file: PathBuf,
}

impl Cli {
    /// Runs the Bitfinity Deployer application.
    ///
    pub async fn run() -> anyhow::Result<()> {
        let cli = Cli::parse();
        let identity = &cli.identity;

        cli.init_tracing();

        let mut canister_manager =
            CanisterManager::load_from_file(&cli.state_file).unwrap_or_else(|_| {
                info!("No existing state file found. Creating a new CanisterManager.");
                CanisterManager::new()
            });

        info!("Starting Bitfinity Deployer v{}", VERSION);
        debug!("IC host: {}", cli.ic_host);

        trace!("Executing command: {:?}", cli.command);
        cli.command
            .run(identity.to_path_buf(), &cli.ic_host, &mut canister_manager)
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
