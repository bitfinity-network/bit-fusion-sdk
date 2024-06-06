use candid::{CandidType, Principal};
use ic_exports::ic_cdk::api::call::CallResult as Result;
use ic_exports::ic_cdk::api::management_canister::http_request::HttpHeader;
use serde::Deserialize;

#[derive(CandidType, Deserialize)]
pub struct RegisterProviderArgs {
    pub cycles_per_call: u64,
    pub credential_path: String,
    pub hostname: String,
    pub credentials_headers: Option<Vec<HttpHeader>>,
    pub chain_id: u64,
    pub cycles_per_message_byte: u64,
}

#[derive(CandidType, Deserialize)]
pub enum RequestResult {
    Ok(String),
    Err(RpcError),
}

#[derive(CandidType, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

#[derive(CandidType, Deserialize)]
pub enum ProviderError {
    TooFewCycles {
        expected: candid::Nat,
        received: candid::Nat,
    },
    MissingRequiredProvider,
    ProviderNotFound,
    NoPermission,
}

#[derive(CandidType, Deserialize)]
pub enum ValidationError {
    CredentialPathNotAllowed,
    HostNotAllowed(String),
    CredentialHeaderNotAllowed,
    UrlParseError(String),
    Custom(String),
    InvalidHex(String),
}

#[derive(CandidType, Deserialize)]
pub enum RejectionCode {
    NoError,
    CanisterError,
    SysTransient,
    DestinationInvalid,
    Unknown,
    SysFatal,
    CanisterReject,
}

#[derive(CandidType, Deserialize)]
pub enum HttpOutcallError {
    IcError {
        code: RejectionCode,
        message: String,
    },
    InvalidHttpJsonRpcResponse {
        status: u16,
        body: String,
        parsing_error: Option<String>,
    },
}

#[derive(CandidType, Deserialize)]
pub enum RpcError {
    JsonRpcError(JsonRpcError),
    ProviderError(ProviderError),
    ValidationError(ValidationError),
    HttpOutcallError(HttpOutcallError),
}

pub async fn register_provider(principal: Principal, args: RegisterProviderArgs) -> Result<(u64,)> {
    ic_exports::ic_cdk::call(principal, "registerProvider", (args,)).await
}
