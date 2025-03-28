use std::borrow::Cow;

use rand::Rng as _;
use testcontainers::core::WaitFor;
use testcontainers::Image;

const NAME: &str = "trufflesuite/ganache";
const TAG: &str = "v7.9.2";

/// Ganache image container
#[derive(Debug, Clone)]
pub struct Ganache {
    args: Vec<String>,
}

impl Default for Ganache {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let chain_id = rng.gen_range(1..5000);

        Self {
            args: vec![
                "--chain.vmErrorsOnRPCResponse=true".to_string(),
                "--wallet.totalAccounts=1".to_string(),
                "--wallet.defaultBalance=100000000".to_string(),
                "-m 'candy maple cake sugar pudding cream honey rich smooth crumble sweet treat'"
                    .to_string(),
                format!("--chain.chainId={}", chain_id),
            ],
        }
    }
}

impl Image for Ganache {
    fn name(&self) -> &str {
        NAME
    }

    fn tag(&self) -> &str {
        TAG
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("RPC Listening on 0.0.0.0:8545")]
    }

    fn cmd(&self) -> impl IntoIterator<Item = impl Into<Cow<'_, str>>> {
        &self.args
    }
}
