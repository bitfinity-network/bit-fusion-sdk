use std::borrow::Cow;

use testcontainers::core::WaitFor;
use testcontainers::Image;

use crate::utils::CHAIN_ID;

const NAME: &str = "trufflesuite/ganache";
const TAG: &str = "v7.9.2";

/// Ganache image container
#[derive(Debug, Clone)]
pub struct Ganache {
    args: Vec<String>,
}

impl Default for Ganache {
    fn default() -> Self {
        Self {
            args: vec![
                "--chain.allowUnlimitedContractSize=true".to_string(),
                "--logging.debug=true".to_string(),
                "--logging.verbose=true".to_string(),
                "--miner.blockTime=0".to_string(),
                "--miner.callGasLimit=0x1fffffffffffff".to_string(),
                "--miner.defaultTransactionGasLimit=0x1fffffffffffff".to_string(),
                "--miner.blockGasLimit=0x1fffffffffffff".to_string(),
                "--miner.defaultGasPrice=0xC016219".to_string(),
                "--chain.vmErrorsOnRPCResponse=true".to_string(),
                "--wallet.totalAccounts=1".to_string(),
                "--wallet.defaultBalance=100000000".to_string(),
                "-m 'candy maple cake sugar pudding cream honey rich smooth crumble sweet treat'"
                    .to_string(),
                format!("--chain.chainId={}", CHAIN_ID),
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
