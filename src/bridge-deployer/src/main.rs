use bridge_deployer::cli::Cli;

#[tokio::main]
async fn main() {
    Cli::run().await.expect("failed to run the tool");
}
