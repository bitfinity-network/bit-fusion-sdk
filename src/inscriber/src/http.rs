use std::borrow::Cow;
use std::collections::HashMap;

use candid::CandidType;
use jsonrpc_core::{Error, Failure, Id, MethodCall, Version};
use serde::Deserialize;
use serde_bytes::ByteBuf;
use serde_json::Value;

mod accessor;
mod rpc;

pub use rpc::*;

/// An enumeration of HTTP status codes.
///
/// This enum represents some common HTTP status codes as defined in RFC 7231.
#[repr(u16)]
pub enum HttpStatusCode {
    Ok = 200,
    BadRequest = 400,
    InternalServerError = 500,
}

/// A request received by the HTTP server.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpRequest {
    /// The HTTP method string.
    pub method: Cow<'static, str>,
    /// The URL that was visited.
    pub url: String,
    /// The request headers.
    pub headers: HashMap<Cow<'static, str>, Cow<'static, str>>,
    /// The request body.
    pub body: ByteBuf,
}

/// A HTTP response.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpResponse {
    /// The HTTP status code.
    pub status_code: u16,
    /// The response header map.
    pub headers: HashMap<Cow<'static, str>, Cow<'static, str>>,
    /// The response body.
    pub body: ByteBuf,
    /// Whether the query call should be upgraded to an update call.
    pub upgrade: Option<bool>,
}

impl HttpResponse {
    /// Constructs a new `HttpResponse`.
    pub fn new(
        status_code: u16,
        headers: HashMap<Cow<'static, str>, Cow<'static, str>>,
        body: ByteBuf,
        upgrade: Option<bool>,
    ) -> Self {
        Self {
            status_code,
            headers,
            body,
            upgrade,
        }
    }

    pub fn error(status_code: u16, message: String) -> Self {
        Self {
            status_code,
            headers: HashMap::new(),
            body: ByteBuf::from(message.into_bytes()),
            upgrade: None,
        }
    }

    pub fn new_failure(
        jsonrpc: Option<Version>,
        id: Id,
        error: Error,
        status_code: HttpStatusCode,
    ) -> Self {
        let failure = Failure { jsonrpc, error, id };
        let body = match serde_json::to_vec(&failure) {
            Ok(bytes) => ByteBuf::from(&bytes[..]),
            Err(e) => ByteBuf::from(e.to_string().as_bytes()),
        };

        Self::new(
            status_code as u16,
            HashMap::from([("content-type".into(), "application/json".into())]),
            body,
            None,
        )
    }

    /// Returns a new `HttpResponse` intended to be used for internal errors.
    pub fn internal_error(e: String) -> Self {
        let body = match serde_json::to_vec(&e) {
            Ok(bytes) => ByteBuf::from(&bytes[..]),
            Err(e) => ByteBuf::from(e.to_string().as_bytes()),
        };

        Self {
            status_code: 500,
            headers: HashMap::from([("content-type".into(), "application/json".into())]),
            body,
            upgrade: None,
        }
    }

    /// Returns an OK response with the given body.
    pub fn ok(body: ByteBuf) -> Self {
        Self::new(
            HttpStatusCode::Ok as u16,
            HashMap::from([("content-type".into(), "application/json".into())]),
            body,
            None,
        )
    }

    /// Upgrade response to update call.
    pub fn upgrade_response() -> Self {
        Self::new(204, HashMap::default(), ByteBuf::default(), Some(true))
    }
}

impl HttpRequest {
    pub fn new(data: Value) -> Self {
        let mut headers = HashMap::new();
        headers.insert("content-type".into(), "application/json".into());
        Self {
            method: "POST".into(),
            url: "".into(),
            headers,
            body: ByteBuf::from(serde_json::to_vec(&data).unwrap()),
        }
    }

    pub fn decode_body(&self) -> Result<MethodCall, Box<HttpResponse>> {
        serde_json::from_slice::<MethodCall>(&self.body).map_err(|_| {
            Box::new(HttpResponse::new_failure(
                Some(Version::V2),
                Id::Null,
                Error::parse_error(),
                HttpStatusCode::BadRequest,
            ))
        })
    }
}

/// Macro that handles returning an HTTP response from a result.
/// If the result is Ok, it returns the value. If it's Err, it returns a 500 error response.
#[macro_export]
macro_rules! http_response {
    ($result:expr) => {{
        match $result {
            Ok(res) => res,
            Err(err) => return HttpResponse::error(500, err.to_string()),
        }
    }};
}
