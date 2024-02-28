use core::fmt;
use std::future::Future;
use std::pin::Pin;

use candid::{CandidType, Principal};
use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use ic_canister_client::IcCanisterClient;
use jsonrpc_core::{Request, Response};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum Clients {
    Canister(IcCanisterClient),
    // todo: add http_outcall client
}

impl Clients {
    pub fn canister(principal: Principal) -> Self {
        Self::Canister(IcCanisterClient::new(principal))
    }
}

impl Client for Clients {
    fn send_rpc_request(
        &self,
        request: Request,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Response>> + Send>> {
        match self {
            Clients::Canister(client) => client.send_rpc_request(request),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub enum EvmLink {
    Http(String),
    Ic(Principal),
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
        }
    }
}

impl EvmLink {
    pub fn get_client(&self) -> EthJsonRpcClient<impl Client> {
        match self {
            EvmLink::Http(_) => todo!(),
            EvmLink::Ic(principal) => EthJsonRpcClient::new(Clients::canister(*principal)),
        }
    }
}
