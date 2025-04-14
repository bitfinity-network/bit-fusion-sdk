use std::time::{Duration, Instant};

use alloy::primitives::B256;
use did::HaltError;
use did::block::ExeResult;
use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("transaction failed: {0}")]
    TxFailed(String),

    #[error("transaction halted: {0:?}")]
    TxHalted(HaltError),

    #[error("transaction timed out")]
    Timeout,
}

pub async fn wait_for_tx(
    client: &EthJsonRpcClient<impl Client>,
    hash: B256,
) -> Result<Vec<u8>, TransactionError> {
    const TX_TIMEOUT: Duration = Duration::from_secs(120);
    const TX_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

    let timeout = Instant::now() + TX_TIMEOUT;
    while Instant::now() < timeout {
        if let Ok(result) = client.get_tx_execution_result_by_hash(hash.into()).await {
            return match result.exe_result {
                ExeResult::Success { output, .. } => match output {
                    did::block::TransactOut::None => Ok(vec![]),
                    did::block::TransactOut::Call(v) => Ok(v),
                    did::block::TransactOut::Create(v, _) => Ok(v),
                },
                ExeResult::Revert { revert_message, .. } => Err(TransactionError::TxFailed(
                    revert_message.unwrap_or_default(),
                )),
                ExeResult::Halt { error, .. } => Err(TransactionError::TxHalted(error)),
            };
        }

        tokio::time::sleep(TX_REQUEST_INTERVAL).await;
    }

    Err(TransactionError::Timeout)
}
