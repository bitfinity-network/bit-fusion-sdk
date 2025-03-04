use alloy::primitives::{keccak256, Address, U256};

pub fn get_contract_address(sender: Address, nonce: impl Into<U256>) -> Address {
    let nonce: U256 = nonce.into();

    let mut stream = rlp::RlpStream::new();
    stream.begin_list(2);
    stream.append(&sender.as_slice());
    stream.append(&nonce.as_le_slice());

    let hash = keccak256(stream.out());

    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    Address::from(bytes)
}
