use cli::Cli;

mod bridge_deployer;
mod cli;
mod commands;
mod config;
mod contracts;
mod evm;

#[tokio::main]
async fn main() {
    Cli::run().await.expect("failed to run the tool");
}
