use candid::{self, CandidType, Deserialize, Principal};
use ic_exports::ic_cdk;
use ic_exports::ic_cdk::api::call::CallResult as Result;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum EthSepoliaService {
    Alchemy,
    BlockPi,
    PublicNode,
    Ankr,
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum L2MainnetService {
    Alchemy,
    BlockPi,
    PublicNode,
    Ankr,
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpHeader {
    pub value: String,
    pub name: String,
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcApi {
    pub url: String,
    pub headers: Option<Vec<HttpHeader>>,
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum EthMainnetService {
    Alchemy,
    BlockPi,
    Cloudflare,
    PublicNode,
    Ankr,
}

#[derive(Debug, Error, CandidType, Deserialize)]
#[error("{message} ({code})")]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Error, CandidType, Deserialize)]
pub enum ProviderError {
    #[error("too few cycles: expected: {expected}, received: {received}")]
    TooFewCycles {
        expected: candid::Nat,
        received: candid::Nat,
    },
    #[error("missing required provider")]
    MissingRequiredProvider,
    #[error("provider not found")]
    ProviderNotFound,
    #[error("no permission")]
    NoPermission,
}

#[derive(Debug, Error, CandidType, Deserialize)]
pub enum ValidationError {
    #[error("credential path not allowed")]
    CredentialPathNotAllowed,
    #[error("host not allowed: {0}")]
    HostNotAllowed(String),
    #[error("credential header not allowed")]
    CredentialHeaderNotAllowed,
    #[error("url parse error: {0}")]
    UrlParseError(String),
    #[error("custom service not allowed: {0}")]
    Custom(String),
    #[error("invalid hex: {0}")]
    InvalidHex(String),
}

#[derive(Debug, Error, CandidType, Deserialize)]
pub enum RejectionCode {
    #[error("no error")]
    NoError,
    #[error("canister error")]
    CanisterError,
    #[error("system error")]
    SysTransient,
    #[error("destination invalid")]
    DestinationInvalid,
    #[error("unknown error")]
    Unknown,
    #[error("system error")]
    SysFatal,
    #[error("canister reject")]
    CanisterReject,
}

#[derive(Debug, Error, CandidType, Deserialize)]
pub enum HttpOutcallError {
    #[error("IC error: {message} ({code})")]
    IcError {
        code: RejectionCode,
        message: String,
    },
    #[error("invalid http json-rpc response: status: {status}, body: {body}, parsing error: {parsing_error:?}")]
    InvalidHttpJsonRpcResponse {
        status: u16,
        body: String,
        parsing_error: Option<String>,
    },
}

#[derive(Debug, Error, CandidType, Deserialize)]
pub enum RpcError {
    #[error("JSON-RPC error: {0}")]
    JsonRpcError(JsonRpcError),
    #[error("provider error: {0}")]
    ProviderError(ProviderError),
    #[error("validation error: {0}")]
    ValidationError(ValidationError),
    #[error("http outcall error: {0}")]
    HttpOutcallError(HttpOutcallError),
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum RpcService {
    EthSepolia(EthSepoliaService),
    BaseMainnet(L2MainnetService),
    Custom(RpcApi),
    OptimismMainnet(L2MainnetService),
    ArbitrumOne(L2MainnetService),
    EthMainnet(EthMainnetService),
    Chain(u64),
    Provider(u64),
}

#[derive(CandidType, Deserialize)]
pub enum RequestResult {
    Ok(String),
    Err(RpcError),
}

#[derive(CandidType, Deserialize)]
pub enum RequestCostResult {
    Ok(candid::Nat),
    Err(RpcError),
}

#[derive(CandidType, Deserialize)]
pub struct UpdateProviderArgs {
    pub cycles_per_call: Option<u64>,
    pub credential_path: Option<String>,
    pub hostname: Option<String>,
    pub credential_headers: Option<Vec<HttpHeader>>,
    pub primary: Option<bool>,
    pub cycles_per_message_byte: Option<u64>,
    pub provider_id: u64,
}

pub struct Service(pub Principal);
impl Service {
    pub async fn request(
        &self,
        rpc_service: &RpcService,
        request: &str,
        max_response_size: u64,
        cycles: u128,
    ) -> Result<(RequestResult,)> {
        ic_cdk::api::call::call_with_payment128(
            self.0,
            "request",
            (rpc_service, request, max_response_size),
            cycles,
        )
        .await
    }
    pub async fn request_cost(
        &self,
        rpc_service: &RpcService,
        request: &str,
        max_response_size: u64,
    ) -> Result<(RequestCostResult,)> {
        ic_cdk::call(
            self.0,
            "requestCost",
            (rpc_service, request, max_response_size),
        )
        .await
    }
}
