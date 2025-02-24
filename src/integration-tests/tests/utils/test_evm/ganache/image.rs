use std::borrow::Cow;

use testcontainers::core::WaitFor;
use testcontainers::Image;

const NAME: &str = "trufflesuite/ganache";
const TAG: &str = "v7.9.2";

/// Ganache image container
#[derive(Debug, Clone)]
pub struct Ganache;

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
        [
            "--chain.vmErrorsOnRPCResponse=true",
            "--wallet.totalAccounts=1",
            "--wallet.defaultBalance=100000000",
            "-m 'candy maple cake sugar pudding cream honey rich smooth crumble sweet treat'",
        ]
    }
}
