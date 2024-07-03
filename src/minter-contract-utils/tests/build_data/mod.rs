use minter_contract_utils::build_data::BFT_BRIDGE_SMART_CONTRACT_CODE;
use solidity_helper::SolidityBuilder;

#[test]
fn test_should_return_the_token_contract_code() {
    // Arrange
    let builder = SolidityBuilder::new().unwrap();
    let contracts = builder.build_updated_contracts().unwrap().contracts;
    let smart_contract = contracts.get("BFTBridge").unwrap();

    // Act
    let smart_token_bytecode = &*BFT_BRIDGE_SMART_CONTRACT_CODE;

    // Assert
    assert_eq!(&smart_contract.bytecode, smart_token_bytecode);
}
