use minter_contract_utils::build_data::BFT_BRIDGE_SMART_CONTRACT_CODE;
use solidity_helper::compile_solidity_contracts;

#[test]
fn test_should_return_the_token_contract_code() {
    // Arrange
    let contracts = compile_solidity_contracts(None, None).unwrap();
    let smart_contract = contracts.get("BFTBridge").unwrap();

    // Act
    let smart_token_bytecode = &*BFT_BRIDGE_SMART_CONTRACT_CODE;

    // Assert
    assert_eq!(&smart_contract.bytecode, smart_token_bytecode);
}
