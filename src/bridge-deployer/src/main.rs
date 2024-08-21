use bridge_deployer::cli::Cli;
use clap::{ArgAction, FromArgMatches, Subcommand};
use clap::{Parser, ValueEnum};
use rune_bridge::state::RuneBridgeConfig;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    Cli::run().await.expect("failed to run CLI");
}
