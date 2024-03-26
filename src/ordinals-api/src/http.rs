use candid::CandidType;
#[cfg(feature = "http-mock")]
use ic_exports::ic_cdk::api::management_canister::http_request::HttpResponse;
#[cfg(not(feature = "http-mock"))]
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpMethod,
};
#[cfg(feature = "http-mock")]
use ic_exports::ic_kit::ic;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "http-mock"))]
const HTTP_OUTCALL_PER_CALL_COST: u128 = 171_360_000;

#[cfg(not(feature = "http-mock"))]
const HTTP_OUTCALL_REQ_PER_BYTE_COST: u128 = 13_600;

#[cfg(not(feature = "http-mock"))]
const HTTP_OUTCALL_RES_PER_BYTE_COST: u128 = 27_200;

#[cfg(not(feature = "http-mock"))]
// 2MiB
const HTTP_OUTCALL_RES_DEFAULT_SIZE: u64 = 2097152;

/// Generic paginated response schema for most api endpoints which may return many results.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct PaginatedResp<T> {
    pub limit: u64,
    pub offset: u64,
    pub total: u64,
    pub results: Vec<T>,
}

#[cfg(not(feature = "http-mock"))]
fn get_estimated_http_outcall_cycles(req: &CanisterHttpRequestArgument) -> u128 {
    let headers_size = req.headers.iter().fold(0u128, |len, header| {
        len + header.value.len() as u128 + header.name.len() as u128
    });

    let mut request_size = req.url.len() as u128 + headers_size;

    if let Some(transform) = &req.transform {
        request_size += transform.context.len() as u128;
    }

    if let Some(body) = &req.body {
        request_size += body.len() as u128;
    }

    let http_outcall_cost: u128 = HTTP_OUTCALL_PER_CALL_COST
        + HTTP_OUTCALL_REQ_PER_BYTE_COST * request_size
        + HTTP_OUTCALL_RES_PER_BYTE_COST
            * req
                .max_response_bytes
                .unwrap_or(HTTP_OUTCALL_RES_DEFAULT_SIZE) as u128;

    http_outcall_cost
}

#[cfg(not(feature = "http-mock"))]
pub async fn http_get_req<T>(url: &str) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    let req = CanisterHttpRequestArgument {
        url: url.to_string(),
        max_response_bytes: None,
        method: HttpMethod::GET,
        headers: Vec::new(),
        body: None,
        transform: None,
    };

    let cycles = get_estimated_http_outcall_cycles(&req);

    let (resp,) = http_request(req, cycles)
        .await
        .map_err(|(_rejection_code, cause)| cause)?;

    if resp.status == 200u16 {
        let data = serde_json::from_slice(&resp.body).map_err(|x| x.to_string())?;

        Ok(Some(data))
    } else if resp.status == 404u16 {
        Ok(None)
    } else {
        Err("Invalid http status code".to_string())
    }
}

#[cfg(feature = "http-mock")]
pub async fn http_get_req<T>(url: &str) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    let canister_id = ic::id();
    let (maybe_resp,): (Option<HttpResponse>,) =
        ic::call(canister_id, "get_http_mock", (url.to_string(),))
            .await
            .map_err(|(_rejection_code, cause)| cause)?;

    let Some(resp) = maybe_resp else {
        panic!("HTTP mock is not found")
    };

    if resp.status == 200u16 {
        let data = serde_json::from_slice(&resp.body).map_err(|x| x.to_string())?;

        Ok(Some(data))
    } else if resp.status == 404u16 {
        Ok(None)
    } else {
        Err("Invalid http status code".to_string())
    }
}
