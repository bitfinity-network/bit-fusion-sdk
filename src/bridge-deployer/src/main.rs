use cli::Cli;

mod cli;
mod commands;
mod config;
mod contracts;

#[tokio::main]
async fn main() {
    Cli::run().await.expect("failed to run the tool");
}
