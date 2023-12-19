use std::future::Future;
use std::pin::Pin;

use candid::{Principal, CandidType};
use ethereum_json_rpc_client::{Client, EthJsonRcpClient};
use ic_canister_client::IcCanisterClient;
use jsonrpc_core::{Request, Response};
use serde::{Serialize, Deserialize};


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

impl EvmLink {
    pub fn get_client(&self) -> EthJsonRcpClient<impl Client> {
        match self {
            EvmLink::Http(_) => todo!(),
            EvmLink::Ic(principal) => EthJsonRcpClient::new(Clients::canister(*principal)),
        }
    }
}