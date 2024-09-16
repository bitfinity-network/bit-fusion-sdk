use std::future::Future;
use std::pin::Pin;

pub use bridge_did::evm_link::{
    EthMainnetService, EthSepoliaService, L2MainnetService, RequestCostResult, RequestResult,
    RpcApi, RpcService, Service,
};
use candid::Principal;
use jsonrpc_core::{Request, Response};
use num_traits::ToPrimitive;

/// Client for sending RPC requests to the EVM-RPC canister.
#[derive(Debug, Clone)]
pub struct EvmRpcCanisterClient {
    principal: Principal,
    rpc_service: Vec<RpcService>,
}

impl EvmRpcCanisterClient {
    /// Creates a new client with the given principal and RPC services to forward requests to.
    pub fn new(principal: Principal, rpc_service: &[RpcService]) -> Self {
        Self {
            principal,
            rpc_service: rpc_service.to_vec(),
        }
    }

    /// Sends an RPC request to the EVM-RPC canister.
    pub fn send_rpc_request(
        &self,
        request: Request,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Response>> + Send>> {
        let rpc_service = self.rpc_service.clone();
        Box::pin(Self::try_rpc_request(self.principal, rpc_service, request))
    }

    /// Tries to send the request with a random service from the list
    /// If it fails, it tries with another service
    async fn try_rpc_request(
        principal: Principal,
        rpc_service: Vec<RpcService>,
        request: Request,
    ) -> anyhow::Result<Response> {
        let request = serde_json::to_string(&request)?;
        let mut last_error = None;
        // shuffle services using timestamp and module
        let time = ic_exports::ic_cdk::api::time();
        for _ in 0..rpc_service.len() {
            let index = time % rpc_service.len() as u64;
            let service = &rpc_service[index as usize];
            match Self::__send_rpc_request(principal, service, &request).await {
                Ok(response) => return Ok(response),
                Err(err) => last_error = Some(err),
            }
        }

        match last_error {
            Some(err) => Err(err),
            None => anyhow::bail!("No services available"),
        }
    }

    /// Sends an RPC request to the EVM-RPC canister using the given service.
    async fn __send_rpc_request(
        principal: Principal,
        rpc_service: &RpcService,
        request: &str,
    ) -> anyhow::Result<Response> {
        let service = Service(principal);
        const MAX_RESPONSE_SIZE: u64 = 2000000;

        // get request cost as cycles
        let (request_cost_result,) = service
            .request_cost(rpc_service, request, MAX_RESPONSE_SIZE)
            .await
            .map_err(|(err, msg)| anyhow::anyhow!("request_cost failed: {err:?}, msg: {msg}",))?;

        let cycles = match request_cost_result {
            RequestCostResult::Ok(cycles) => cycles
                .0
                .to_u128()
                .ok_or_else(|| anyhow::anyhow!("cycles conversion failed"))?,
            RequestCostResult::Err(err) => {
                anyhow::bail!("request_cost error: {err}");
            }
        };

        // send rpc request
        let (request_result,) = service
            .request(rpc_service, request, MAX_RESPONSE_SIZE, cycles)
            .await
            .map_err(|(err, msg)| anyhow::anyhow!("request failed: {err:?}, msg: {msg}",))?;

        let response = match request_result {
            RequestResult::Ok(response) => serde_json::from_str(&response)?,
            RequestResult::Err(err) => {
                anyhow::bail!("request error: {err}");
            }
        };

        Ok(response)
    }
}
