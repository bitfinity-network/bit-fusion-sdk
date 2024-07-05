mod evm_rpc_canister_client;

use core::fmt;
use std::future::Future;
use std::pin::Pin;

use candid::{CandidType, Principal};
use ethereum_json_rpc_client::http_outcall::HttpOutcallClient;
use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use ethers_core::types::H160;
use ic_canister_client::IcCanisterClient;
use jsonrpc_core::{Request, Response};
use serde::{Deserialize, Serialize};

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
        Self::HttpOutCall(HttpOutcallClient::new(url))
    }

    pub fn evm_rpc_canister(principal: Principal, rpc_service: &[RpcService]) -> Self {
        Self::EvmRpcCanister(EvmRpcCanisterClient::new(principal, rpc_service))
    }
}

impl Client for Clients {
    fn send_rpc_request(
        &self,
        request: Request,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Response>> + Send>> {
        match self {
            Clients::Canister(client) => client.send_rpc_request(request),
            Clients::HttpOutCall(client) => client.send_rpc_request(request),
            Clients::EvmRpcCanister(client) => client.send_rpc_request(request),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub enum EvmLink {
    Http(String),
    Ic(Principal),
    EvmRpcCanister {
        canister_id: Principal,
        rpc_service: Vec<RpcService>,
    },
}

impl Default for EvmLink {
    fn default() -> Self {
        EvmLink::Ic(Principal::anonymous())
    }
}

impl fmt::Display for EvmLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvmLink::Http(url) => write!(f, "Http EVM link: {url}"),
            EvmLink::Ic(principal) => write!(f, "Ic EVM link: {principal}"),
            EvmLink::EvmRpcCanister {
                canister_id: principal,
                rpc_service,
            } => {
                write!(f, "EVM RPC link: {principal}, {rpc_service:?}")
            }
        }
    }
}

impl EvmLink {
    /// Returns the JSON-RPC client.
    pub fn get_json_rpc_client(&self) -> EthJsonRpcClient<impl Client> {
        match self {
            EvmLink::Http(url) => EthJsonRpcClient::new(Clients::http_outcall(url.clone())),
            EvmLink::Ic(principal) => EthJsonRpcClient::new(Clients::canister(*principal)),
            EvmLink::EvmRpcCanister {
                canister_id: principal,
                rpc_service,
            } => EthJsonRpcClient::new(Clients::evm_rpc_canister(*principal, rpc_service)),
        }
    }

    /// Returns the underlying client.
    pub fn get_client(&self) -> impl Client {
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

pub fn address_to_icrc_subaccount(address: &H160) -> [u8; 32] {
    let mut subaccount = [0u8; 32];
    subaccount[..20].copy_from_slice(address.as_bytes());
    subaccount
}
