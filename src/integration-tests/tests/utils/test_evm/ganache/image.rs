use std::borrow::Cow;

use testcontainers::core::WaitFor;
use testcontainers::Image;

const NAME: &str = "trufflesuite/ganache";
const TAG: &str = "v7.9.2";

//pub const MINTER_ACCOUNT_PRIVKEY: &str =
//    "0xc87509a1c067bbde78beb793e6fa76530b6382a4c0241e5e4a9ec0a0f44dc0d3";
//pub const MINTER_ACCOUNT_ADDRESS: &str = "0x627306090abaB3A6e1400e9345bC60c78a8BEf57";

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
