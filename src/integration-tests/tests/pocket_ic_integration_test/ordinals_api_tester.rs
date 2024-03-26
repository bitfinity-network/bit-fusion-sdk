use ic_canister_client::PocketIcClient;
use ic_exports::ic_cdk::api::management_canister::http_request::HttpResponse;
use ordinals_api::{brc20, http, inscription};

use super::PocketIcTestContext;
use crate::context::{CanisterType, TestContext};

async fn set_http_mock(client: &PocketIcClient, url: String, resp: HttpResponse) {
    client
        .update::<(String, HttpResponse), ()>("set_http_mock", (url, resp))
        .await
        .expect("Can't set http mock");
}

#[tokio::test]
async fn get_base_api_url() {
    let ctx = PocketIcTestContext::new(&[CanisterType::OrdinalsApiTester]).await;
    let ordinals_api_address = ctx.canisters.ordinals_api_tester();
    let client = ctx.client(ordinals_api_address, ctx.admin_name());

    let resp = client
        .query::<(), String>("get_base_api_url", ())
        .await
        .expect("Can't obtain base api url");

    assert_eq!(resp, String::from("http://localhost:3000"));
}

#[tokio::test]
async fn get_brc20_tokens() {
    let ctx = PocketIcTestContext::new(&[CanisterType::OrdinalsApiTester]).await;
    let ordinals_api_address = ctx.canisters.ordinals_api_tester();
    let client = ctx.client(ordinals_api_address, ctx.admin_name());

    let mock_data_json: serde_json::Value = serde_json::from_str(
        r#"
    {
        "limit": 3,
        "offset": 1,
        "total": 66144,
        "results": [
            {
                "id": "62ed83b59e36248eeb84a1fd57b468a156093e013ca43e2226fad77fd6427700i0",
                "number": 65964114,
                "block_height": 836401,
                "tx_id": "62ed83b59e36248eeb84a1fd57b468a156093e013ca43e2226fad77fd6427700",
                "address": "bc1pktk943q0cc6kge9qmkn4yvyhnwg6j2g8p09qv36tmjukzjvvgnfqtezs3y",
                "ticker": "JKTO",
                "max_supply": "21000000.000000000000000000",
                "mint_limit": "10.000000000000000000",
                "decimals": 18,
                "deploy_timestamp": 1711475996000,
                "minted_supply": "0.000000000000000000",
                "tx_count": 1
            },
            {
                "id": "81bcb693e8fca8577d1b43710b6817af8a6dd16b1b592f097b8f68b9d81593e8i0",
                "number": 65962447,
                "block_height": 836400,
                "tx_id": "81bcb693e8fca8577d1b43710b6817af8a6dd16b1b592f097b8f68b9d81593e8",
                "address": "bc1pdr05way9xue9kx08lqmuz8293zt0qmecxm4zcvd5mazjc4ychmssqfq7c2",
                "ticker": "X49C",
                "max_supply": "21000000.000000000000000000",
                "mint_limit": "1000000.000000000000000000",
                "decimals": 18,
                "deploy_timestamp": 1711475222000,
                "minted_supply": "0.000000000000000000",
                "tx_count": 1
            },
            {
                "id": "70f9c489d0355bfab8ad168d3ae23c1e831b647fb919db65facf1a5e04a25e87i0",
                "number": 65960579,
                "block_height": 836398,
                "tx_id": "70f9c489d0355bfab8ad168d3ae23c1e831b647fb919db65facf1a5e04a25e87",
                "address": "bc1pg9sa4ktcwk6vpaf9n5ehhgtmx8qnldt75c9xad3fa3lwv9ezx2ss4g6lm4",
                "ticker": "WTFy",
                "max_supply": "21000000.000000000000000000",
                "mint_limit": "1500000.000000000000000000",
                "decimals": 18,
                "deploy_timestamp": 1711474593000,
                "minted_supply": "0.000000000000000000",
                "tx_count": 1
            }
        ]
    }"#,
    )
    .unwrap();

    set_http_mock(
        &client,
        "http://localhost:3000/ordinals/v1/brc-20/tokens?offset=1&limit=3".to_string(),
        HttpResponse {
            status: 200u16.into(),
            headers: Vec::new(),
            body: serde_json::to_vec(&mock_data_json).unwrap(),
        },
    )
    .await;

    let resp = client
        .update::<(u64, u64), Option<http::PaginatedResp<brc20::state::Brc20Token>>>(
            "get_brc20_tokens",
            (1, 3),
        )
        .await
        .expect("Can't get brc20 token by ticker")
        .unwrap();

    assert_eq!(resp.limit, 3);
    assert_eq!(resp.offset, 1);
    assert_eq!(resp.results.len(), 3);
}

#[tokio::test]
async fn get_brc20_token_by_ticker() {
    let ctx = PocketIcTestContext::new(&[CanisterType::OrdinalsApiTester]).await;
    let ordinals_api_address = ctx.canisters.ordinals_api_tester();
    let client = ctx.client(ordinals_api_address, ctx.admin_name());

    let mock_data_json: serde_json::Value = serde_json::from_str(
        r#"{
        "token": {
            "id": "b61b0172d95e266c18aea0c624db987e971a5d6d4ebc2aaed85da4642d635735i0",
            "number": 348020,
            "block_height": 779832,
            "tx_id": "b61b0172d95e266c18aea0c624db987e971a5d6d4ebc2aaed85da4642d635735",
            "address": "bc1pxaneaf3w4d27hl2y93fuft2xk6m4u3wc4rafevc6slgd7f5tq2dqyfgy06",
            "ticker": "ordi",
            "max_supply": "21000000.000000000000000000",
            "mint_limit": "1000.000000000000000000",
            "decimals": 18,
            "deploy_timestamp": 1678248991000,
            "minted_supply": "21000000.000000000000000000",
            "tx_count": 272808
        },
        "supply": {
            "max_supply": "21000000.000000000000000000",
            "minted_supply": "21000000.000000000000000000",
            "holders": 17961
        }
    }"#,
    )
    .unwrap();

    set_http_mock(
        &client,
        "http://localhost:3000/ordinals/v1/brc-20/tokens/ordi".to_string(),
        HttpResponse {
            status: 200u16.into(),
            headers: Vec::new(),
            body: serde_json::to_vec(&mock_data_json).unwrap(),
        },
    )
    .await;

    let resp = client
        .update::<(String,), Option<brc20::state::Brc20TokenDetails>>(
            "get_brc20_token_by_ticker",
            (String::from("ordi"),),
        )
        .await
        .expect("Can't get brc20 token by ticker")
        .unwrap();

    assert_eq!(resp.token.ticker, "ordi".to_string());
    assert_eq!(resp.token.tx_count, 272808);
    assert_eq!(
        resp.token.id,
        "b61b0172d95e266c18aea0c624db987e971a5d6d4ebc2aaed85da4642d635735i0".to_string()
    );
}

#[tokio::test]
async fn get_brc20_token_holders_by_ticker() {
    let ctx = PocketIcTestContext::new(&[CanisterType::OrdinalsApiTester]).await;
    let ordinals_api_address = ctx.canisters.ordinals_api_tester();
    let client = ctx.client(ordinals_api_address, ctx.admin_name());

    let mock_data_json: serde_json::Value = serde_json::from_str(
        r#"
    {
        "limit": 3,
        "offset": 1,
        "total": 43772,
        "results": [
            {
                "address": "bc1qggf48ykykz996uv5vsp5p9m9zwetzq9run6s64hm6uqfn33nhq0ql9t85q",
                "overall_balance": "1676295.447495600000000000"
            },
            {
                "address": "bc1qm64dsdz853ntzwleqsrdt5p53w75zfrtnmyzcx",
                "overall_balance": "1461460.627277140000000000"
            },
            {
                "address": "bc1qqd72vtqlw0nugqmzrx398x8gj03z8aqr79aexrncezqaw74dtu4qxjydq3",
                "overall_balance": "989780.514209670000000000"
            }
        ]
    }"#,
    )
    .unwrap();

    set_http_mock(
        &client,
        "http://localhost:3000/ordinals/v1/brc-20/tokens/ordi/holders?offset=1&limit=3".to_string(),
        HttpResponse {
            status: 200u16.into(),
            headers: Vec::new(),
            body: serde_json::to_vec(&mock_data_json).unwrap(),
        },
    )
    .await;

    let resp = client
        .update::<(String, u64, u64), Option<http::PaginatedResp<brc20::state::Brc20Holder>>>(
            "get_brc20_token_holders_by_ticker",
            (String::from("ordi"), 1, 3),
        )
        .await
        .expect("Can't get brc20 token by ticker")
        .unwrap();

    assert_eq!(resp.limit, 3);
    assert_eq!(resp.offset, 1);
    assert_eq!(resp.total, 43772);
    assert_eq!(resp.results.len(), 3);
}

#[tokio::test]
async fn get_brc20_token_balance_by_address() {
    let ctx = PocketIcTestContext::new(&[CanisterType::OrdinalsApiTester]).await;
    let ordinals_api_address = ctx.canisters.ordinals_api_tester();
    let client = ctx.client(ordinals_api_address, ctx.admin_name());

    let mock_data_json: serde_json::Value = serde_json::from_str(
        r#"
    {
        "limit": 20,
        "offset": 0,
        "total": 1,
        "results": [
            {
                "ticker": "ordi",
                "available_balance": "943.447495600000000000",
                "transferrable_balance": "1675352.000000000000000000",
                "overall_balance": "1676295.447495600000000000"
            }
        ]
    }"#,
    )
    .unwrap();

    set_http_mock(
        &client,
        "http://localhost:3000/ordinals/v1/brc-20/balances/bc1qggf48ykykz996uv5vsp5p9m9zwetzq9run6s64hm6uqfn33nhq0ql9t85q?ticker=ordi".to_string(),
        HttpResponse {
            status: 200u16.into(),
            headers: Vec::new(),
            body: serde_json::to_vec(&mock_data_json).unwrap(),
        },
    )
    .await;

    let resp = client
        .update::<(String, String), Option<http::PaginatedResp<brc20::state::Brc20Balance>>>(
            "get_brc20_token_balance_by_address",
            (
                String::from("bc1qggf48ykykz996uv5vsp5p9m9zwetzq9run6s64hm6uqfn33nhq0ql9t85q"),
                String::from("ordi"),
            ),
        )
        .await
        .expect("Can't get brc20 token by ticker")
        .unwrap();

    assert_eq!(resp.results.len(), 1);
    assert_eq!(
        resp.results[0].overall_balance,
        "1676295.447495600000000000".to_string()
    );
}

#[tokio::test]
async fn get_inscription_by_id() {
    let ctx = PocketIcTestContext::new(&[CanisterType::OrdinalsApiTester]).await;
    let ordinals_api_address = ctx.canisters.ordinals_api_tester();
    let client = ctx.client(ordinals_api_address, ctx.admin_name());

    let mock_data_json: serde_json::Value = serde_json::from_str(
        r#"
    {
        "id": "c8206d089f8bf5f3995ee4775c7bb7f949eb68c5fc2a650d5eb364b78dc1663fi1",
        "number": 65966378,
        "address": "bc1pcyv3w66zkcr9c990c957vxquj6vsr8s5ddpyc8g2yxae3c3rmlqs6jef04",
        "genesis_address": "bc1pcyv3w66zkcr9c990c957vxquj6vsr8s5ddpyc8g2yxae3c3rmlqs6jef04",
        "genesis_block_height": 836403,
        "genesis_block_hash": "000000000000000000003d440b568029d1554d70e126635e4c7b0f82c87cd515",
        "genesis_tx_id": "c8206d089f8bf5f3995ee4775c7bb7f949eb68c5fc2a650d5eb364b78dc1663f",
        "genesis_fee": "96031",
        "genesis_timestamp": 1711476778000,
        "tx_id": "c8206d089f8bf5f3995ee4775c7bb7f949eb68c5fc2a650d5eb364b78dc1663f",
        "location": "c8206d089f8bf5f3995ee4775c7bb7f949eb68c5fc2a650d5eb364b78dc1663f:1:0",
        "output": "c8206d089f8bf5f3995ee4775c7bb7f949eb68c5fc2a650d5eb364b78dc1663f:1",
        "value": "546",
        "offset": "0",
        "sat_ordinal": "833805483854547",
        "sat_rarity": "common",
        "sat_coinbase_height": 166761,
        "mime_type": "image/webp",
        "content_type": "image/webp",
        "content_length": 13678,
        "timestamp": 1711476778000,
        "curse_type": "\"NotAtOffsetZero\"",
        "recursive": false,
        "recursion_refs": null
    }"#,
    )
    .unwrap();

    set_http_mock(
        &client,
        "http://localhost:3000/ordinals/v1/inscriptions/c8206d089f8bf5f3995ee4775c7bb7f949eb68c5fc2a650d5eb364b78dc1663fi1".to_string(),
        HttpResponse {
            status: 200u16.into(),
            headers: Vec::new(),
            body: serde_json::to_vec(&mock_data_json).unwrap(),
        },
    )
    .await;

    let resp = client
        .update::<(String,), Option<inscription::state::Inscription>>(
            "get_inscription_by_id",
            (String::from(
                "c8206d089f8bf5f3995ee4775c7bb7f949eb68c5fc2a650d5eb364b78dc1663fi1",
            ),),
        )
        .await
        .expect("Can't get inscription by id")
        .unwrap();

    assert_eq!(
        resp.id,
        "c8206d089f8bf5f3995ee4775c7bb7f949eb68c5fc2a650d5eb364b78dc1663fi1".to_string()
    );
    assert_eq!(resp.number, 65966378);
    assert_eq!(resp.mime_type, "image/webp".to_string());
}

#[tokio::test]
async fn get_inscription_transfers_by_id() {
    let ctx = PocketIcTestContext::new(&[CanisterType::OrdinalsApiTester]).await;
    let ordinals_api_address = ctx.canisters.ordinals_api_tester();
    let client = ctx.client(ordinals_api_address, ctx.admin_name());

    let mock_data_json: serde_json::Value =
        serde_json::from_str(r#"{"limit":3,"offset":1,"total":0,"results":[]}"#).unwrap();

    set_http_mock(
        &client,
        "http://localhost:3000/ordinals/v1/inscriptions/c8206d089f8bf5f3995ee4775c7bb7f949eb68c5fc2a650d5eb364b78dc1663fi1/transfers?offset=1&limit=3".to_string(),
        HttpResponse {
            status: 200u16.into(),
            headers: Vec::new(),
            body: serde_json::to_vec(&mock_data_json).unwrap(),
        },
    )
    .await;

    let resp = client
        .update::<(String, u64, u64,), Option<http::PaginatedResp<inscription::state::InscriptionLocation>>>(
            "get_inscription_transfers_by_id",
            (String::from("c8206d089f8bf5f3995ee4775c7bb7f949eb68c5fc2a650d5eb364b78dc1663fi1"), 1, 3),
        )
        .await
        .expect("Can't get inscription transfers by id")
        .unwrap();

    assert_eq!(resp.limit, 3);
    assert_eq!(resp.offset, 1);
    assert_eq!(resp.total, 0);
    assert_eq!(resp.results.len(), 0);
}
