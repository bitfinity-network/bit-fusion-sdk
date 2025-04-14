mod evm_rpc_canister_client;

use std::future::Future;
use std::pin::Pin;

use alloy::primitives::Address;
use bridge_did::evm_link::EvmLink;
use candid::Principal;
use did::rpc::request::RpcRequest;
use did::rpc::response::RpcResponse;
use ethereum_json_rpc_client::http_outcall::HttpOutcallClient;
use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use ic_canister_client::IcCanisterClient;

use self::evm_rpc_canister_client::EvmRpcCanisterClient;
pub use self::evm_rpc_canister_client::{
    EthMainnetService, EthSepoliaService, L2MainnetService, RpcApi, RpcService,
};

#[derive(Debug, Clone)]
pub enum Clients {
    Canister(IcCanisterClient),
    HttpOutCall(HttpOutcallClient),
    EvmRpcCanister(EvmRpcCanisterClient),
}

impl Clients {
    pub fn canister(principal: Principal) -> Self {
        Self::Canister(IcCanisterClient::new(principal))
    }

    pub fn http_outcall(url: String) -> Self {
        Self::HttpOutCall(HttpOutcallClient::new(url).sanitized())
    }

    pub fn evm_rpc_canister(principal: Principal, rpc_service: &[RpcService]) -> Self {
        Self::EvmRpcCanister(EvmRpcCanisterClient::new(principal, rpc_service))
    }
}

impl Client for Clients {
    fn send_rpc_request(
        &self,
        request: RpcRequest,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<RpcResponse>> + Send>> {
        match self {
            Clients::Canister(client) => client.send_rpc_request(request),
            Clients::HttpOutCall(client) => client.send_rpc_request(request),
            Clients::EvmRpcCanister(client) => client.send_rpc_request(request),
        }
    }
}

pub trait EvmLinkClient {
    /// Returns the JSON-RPC client.
    fn get_json_rpc_client(&self) -> EthJsonRpcClient<impl Client>;

    /// Returns the underlying client.
    fn get_client(&self) -> impl Client;
}

impl EvmLinkClient for EvmLink {
    /// Returns the JSON-RPC client.
    fn get_json_rpc_client(&self) -> EthJsonRpcClient<impl Client> {
        match self {
            EvmLink::Http(url) => {
                log::trace!("Using http client with url: {url}");
                EthJsonRpcClient::new(Clients::http_outcall(url.clone()))
            }
            EvmLink::Ic(principal) => {
                log::trace!("Using IC client with principal: {principal}");
                EthJsonRpcClient::new(Clients::canister(*principal))
            }
            EvmLink::EvmRpcCanister {
                canister_id: principal,
                rpc_service,
            } => {
                log::trace!(
                    "Using rpc client with canister_id: {principal} and rpc_service: {rpc_service:?}"
                );
                EthJsonRpcClient::new(Clients::evm_rpc_canister(*principal, rpc_service))
            }
        }
    }

    /// Returns the underlying client.
    fn get_client(&self) -> impl Client {
        match self {
            EvmLink::Http(url) => Clients::http_outcall(url.clone()),
            EvmLink::Ic(principal) => Clients::canister(*principal),
            EvmLink::EvmRpcCanister {
                canister_id: principal,
                rpc_service,
            } => Clients::evm_rpc_canister(*principal, rpc_service),
        }
    }
}

pub fn address_to_icrc_subaccount(address: &Address) -> [u8; 32] {
    let mut subaccount = [0u8; 32];
    subaccount[..20].copy_from_slice(address.as_slice());
    subaccount
}
