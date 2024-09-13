use did::H160;

use crate::context::{CanisterType, TestContext};

use super::PocketIcTestContext;


#[tokio::test]
async fn generates_correct_deposit_address() {
    
    const ETH_ADDRESS: &str = "0x4e37fc8684e0f7ad6a6c1178855450294a16b418";
    let eth_address = H160::from_hex_str(ETH_ADDRESS).unwrap();
    
    let context = PocketIcTestContext::new(&[CanisterType::RuneBridge]).await;
    let rune_bridge_client = context.rune_bridge_client("alice");

    let address = rune_bridge_client.get_deposit_address(&eth_address).await.unwrap().unwrap();

    let expected = "bc1qq9c8dh2w7vk25644y3ulf808vyggx9z8c6tapp".to_string();

    assert_eq!(
        address,
        expected
    );

    const ANOTHER_ETH_ADDRESS: &str = "0x4e37fc8684e0f7ad6a6c1178855450294a16b419";
    let eth_address = H160::from_hex_str(ANOTHER_ETH_ADDRESS).unwrap();

    let address = rune_bridge_client.get_deposit_address(&eth_address).await.unwrap().unwrap();

    assert_ne!(
        address,
        expected
    );

}
