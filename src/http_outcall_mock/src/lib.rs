mod http_outcall_client;

use http_outcall_client::{HttpOutcallClient, HttpOutcallClientGeneric};
use ic_cdk::api::management_canister::http_request::*;
use std::cell::RefCell;

thread_local! {
    static HTTP_OUTCALL_CLIENT: RefCell<HttpOutcallClientGeneric> = RefCell::new(HttpOutcallClientGeneric::new());
}

#[cfg(feature = "pocket_ic_test")]
#[ic_cdk::update]
fn set_http_outcall_mock(url: String, resp: HttpResponse) {
    HTTP_OUTCALL_CLIENT.with_borrow_mut(|http| http.set_expected_mock(&url, &resp));
}

#[ic_cdk::update]
async fn get_icp_usd_rate() -> String {
    let url =
        "https://api.coingecko.com/api/v3/simple/price?ids=internet-computer&vs_currencies=usd"
            .to_string();

    let req = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::GET,
        body: None,
        max_response_bytes: None,
        transform: None,
        headers: vec![HttpHeader {
            name: "Host".to_string(),
            value: "api.coingecko.com:443".to_string(),
        }],
    };

    let http = HTTP_OUTCALL_CLIENT.with_borrow(|http| http.clone());
    match http.call(req).await {
        Ok((resp,)) => String::from_utf8(resp.body).expect("Unable to decode UTF-8 string"),
        Err((code, message)) => {
            format!("{code:?} - {message}")
        }
    }
}

#[ic_cdk::query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}
