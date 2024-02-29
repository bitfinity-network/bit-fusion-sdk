use candid::CandidType;
use ic_cdk::api::{call::CallResult, management_canister::http_request::*};
#[cfg(feature = "pocket_ic_test")]
use std::collections::HashMap;

#[async_trait::async_trait]
pub trait HttpOutcallClient: Clone {
    async fn call(&self, req: CanisterHttpRequestArgument) -> CallResult<(HttpResponse,)>;

    #[cfg(feature = "pocket_ic_test")]
    fn set_expected_mock(&mut self, url: &str, resp: &HttpResponse);
}

/// Generic impl over [`HttpOutcallClient`]. Construct abstract dynamic dispatched [`HttpOutcallClient`] object.
/// Based on `pocket_ic_test` feature.
#[derive(serde::Serialize, serde::Deserialize, CandidType, Clone)]
pub struct HttpOutcallClientGeneric {
    #[cfg(not(feature = "pocket_ic_test"))]
    client: HttpOutcallClientCanister,
    #[cfg(feature = "pocket_ic_test")]
    client: HttpOutcallClientMock,
}

impl HttpOutcallClientGeneric {
    #[cfg(not(feature = "pocket_ic_test"))]
    pub fn new() -> Self {
        Self {
            client: HttpOutcallClientCanister {},
        }
    }

    #[cfg(feature = "pocket_ic_test")]
    pub fn new() -> Self {
        Self {
            client: HttpOutcallClientMock {
                expected_mock: HashMap::new(),
            },
        }
    }
}

#[async_trait::async_trait]
impl HttpOutcallClient for HttpOutcallClientGeneric {
    async fn call(&self, req: CanisterHttpRequestArgument) -> CallResult<(HttpResponse,)> {
        self.client.call(req).await
    }

    #[cfg(feature = "pocket_ic_test")]
    fn set_expected_mock(&mut self, url: &str, resp: &HttpResponse) {
        self.client.set_expected_mock(url, resp)
    }
}

#[cfg(not(feature = "pocket_ic_test"))]
#[derive(serde::Serialize, serde::Deserialize, CandidType, Clone)]
pub struct HttpOutcallClientCanister {}

#[cfg(not(feature = "pocket_ic_test"))]
#[async_trait::async_trait]
impl HttpOutcallClient for HttpOutcallClientCanister {
    async fn call(&self, req: CanisterHttpRequestArgument) -> CallResult<(HttpResponse,)> {
        // TODO: This should be calculated in runtime
        let cycles = 230_949_972_000;
        http_request(req, cycles).await
    }
}

#[cfg(feature = "pocket_ic_test")]
#[derive(serde::Serialize, serde::Deserialize, CandidType, Clone)]
pub struct HttpOutcallClientMock {
    /// Schema: URL -> Response.
    expected_mock: HashMap<String, HttpResponse>,
}

#[cfg(feature = "pocket_ic_test")]
#[async_trait::async_trait]
impl HttpOutcallClient for HttpOutcallClientMock {
    async fn call(&self, req: CanisterHttpRequestArgument) -> CallResult<(HttpResponse,)> {
        if let Some(mocked_resp) = self.expected_mock.get(&req.url) {
            Ok((mocked_resp.clone(),))
        } else {
            Ok((HttpResponse {
                status: 404u32.into(),
                headers: Vec::new(),
                body: String::from("Mock is not found").as_bytes().to_vec(),
            },))
        }
    }

    fn set_expected_mock(&mut self, url: &str, resp: &HttpResponse) {
        self.expected_mock.insert(url.to_string(), resp.clone());
    }
}
