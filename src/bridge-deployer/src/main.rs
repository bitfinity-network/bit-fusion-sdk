use cli::Cli;

mod bridge_deployer;
mod canister_ids;
mod cli;
mod commands;
mod config;
mod contracts;
mod evm;
mod utils;

#[tokio::main]
async fn main() {
    Cli::run().await.expect("failed to run the tool");
}
