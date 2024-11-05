use bridge_did::error::BTFResult;
use bridge_did::op_id::OperationId;

use super::BridgeService;
use crate::runtime::state::config::ConfigStorage;
use crate::runtime::state::SharedConfig;

/// Service to refresh EVM params in the given config.
pub struct RefreshEvmParamsService {
    config: SharedConfig,
}

impl RefreshEvmParamsService {
    pub fn new(config: SharedConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait(?Send)]
impl BridgeService for RefreshEvmParamsService {
    async fn run(&self) -> BTFResult<()> {
        ConfigStorage::refresh_evm_params(self.config.clone()).await
    }

    fn push_operation(&self, _: OperationId) -> BTFResult<()> {
        let msg = "Operations should not be pushed to the RefreshEvmParamsService service";
        log::warn!("{msg}");
        Err(bridge_did::error::Error::FailedToProgress(msg.into()))
    }
}
