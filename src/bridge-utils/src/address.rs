use alloy::primitives::{keccak256, Address, U256};

/// The address for an Ethereum contract is deterministically computed from the address of its
/// creator (sender) and how many transactions the creator has sent (nonce).
/// The sender and nonce are RLP encoded and then hashed with Keccak-256.
pub fn get_contract_address(sender: Address, nonce: impl Into<U256>) -> Address {
    let nonce: U256 = nonce.into();

    let mut stream = rlp::RlpStream::new();
    stream.begin_list(2);
    stream.append(&sender.as_slice());
    stream.append(&nonce.to_le_bytes_trimmed_vec());

    let out = stream.out();
    let hash = keccak256(&out);

    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    Address::from(bytes)
}

#[cfg(test)]
mod test {

    use std::str::FromStr as _;

    use super::*;

    #[test]
    fn test_should_get_contract_address() {
        let address = Address::from_str("0xe57e761aa806c9afe7e06fb0601b17bec310f9c4")
            .expect("Invalid address");
        let nonce = 1u64;

        let alloy_contract_address = get_contract_address(address, U256::from(nonce));

        let expected = Address::from_str("0x8eEcF5e011C88bdEe7328Df7aE54D7e03cBbb977")
            .expect("Invalid address");

        assert_eq!(alloy_contract_address, expected);
    }
}
