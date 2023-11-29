use std::cell::RefCell;
use std::rc::Rc;

use did::{H160, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::abi::Token;
use minter_contract_utils::bft_bridge_api::MINTER_CANISTER_ADDRESS;
use minter_contract_utils::build_data::BFT_BRIDGE_SMART_CONTRACT_DEPLOYED_CODE;
use minter_did::error::{Error, Result};

use crate::constant::DEFAULT_TX_GAS_LIMIT;
use crate::context::Context;
use crate::evm::Evm;

/// This structure contains data of a valid burn operation.
///
/// The `Src` and `Dst` generic types are useful, because in different contexts
/// we either know concrete types (H160, Principal, ...) or not (Id256).
#[derive(Debug)]
pub struct ValidBurn<Src, Dst> {
    pub amount: U256,
    pub sender: Src,
    pub src_token: Src,
    pub sender_chain_id: u32,
    pub recipient: Dst,
    pub to_token: Dst,
    pub recipient_chain_id: u32,
    pub nonce: u32,
    pub name: [u8; 32],
    pub symbol: [u8; 16],
    pub decimals: u8,
}

impl<Src, Dst> ValidBurn<Src, Dst> {
    /// Convert `sender` and `src_token` fields to `NewSrc` type.
    pub fn map_src<NewSrc, F>(self, mut f: F) -> ValidBurn<NewSrc, Dst>
    where
        F: FnMut(Src) -> NewSrc,
    {
        ValidBurn {
            amount: self.amount,
            sender: f(self.sender),
            src_token: f(self.src_token),
            sender_chain_id: self.sender_chain_id,
            recipient: self.recipient,
            to_token: self.to_token,
            recipient_chain_id: self.recipient_chain_id,
            nonce: self.nonce,
            name: self.name,
            symbol: self.symbol,
            decimals: self.decimals,
        }
    }

    /// Try to convert `sender` and `src_token` fields to `NewSrc` type.
    pub fn try_map_src<NewSrc, F>(self, mut f: F) -> Result<ValidBurn<NewSrc, Dst>>
    where
        F: FnMut(Src) -> Result<NewSrc>,
    {
        Ok(ValidBurn {
            amount: self.amount,
            sender: f(self.sender)?,
            src_token: f(self.src_token)?,
            sender_chain_id: self.sender_chain_id,
            recipient: self.recipient,
            to_token: self.to_token,
            recipient_chain_id: self.recipient_chain_id,
            nonce: self.nonce,
            name: self.name,
            symbol: self.symbol,
            decimals: self.decimals,
        })
    }

    /// Convert `recipient` and `dst_token` fields to `NewDst` type.
    pub fn map_dst<NewDst, F>(self, mut f: F) -> ValidBurn<Src, NewDst>
    where
        F: FnMut(Dst) -> NewDst,
    {
        ValidBurn {
            amount: self.amount,
            sender: self.sender,
            src_token: self.src_token,
            sender_chain_id: self.sender_chain_id,
            recipient: f(self.recipient),
            to_token: f(self.to_token),
            recipient_chain_id: self.recipient_chain_id,
            nonce: self.nonce,
            name: self.name,
            symbol: self.symbol,
            decimals: self.decimals,
        }
    }

    /// Try to convert `recipient` and `dst_token` fields to `NewDst` type.
    pub fn try_map_dst<NewDst, F>(self, mut f: F) -> Result<ValidBurn<Src, NewDst>>
    where
        F: FnMut(Dst) -> Result<NewDst>,
    {
        Ok(ValidBurn {
            amount: self.amount,
            sender: self.sender,
            src_token: self.src_token,
            sender_chain_id: self.sender_chain_id,
            recipient: f(self.recipient)?,
            to_token: f(self.to_token)?,
            recipient_chain_id: self.recipient_chain_id,
            nonce: self.nonce,
            name: self.name,
            symbol: self.symbol,
            decimals: self.decimals,
        })
    }
}

/// Checks if `evm` has valid BftBridge contract deployed.
pub async fn check_bft_bridge_contract(
    evm: &dyn Evm,
    bft_address: H160,
    context: &Rc<RefCell<dyn Context>>,
) -> Result<()> {
    // Check Bft Bridge code
    let code = evm.get_contract_code(bft_address.clone(), context).await?;
    if code != *BFT_BRIDGE_SMART_CONTRACT_DEPLOYED_CODE {
        return Err(Error::InvalidBftBridgeContract);
    }

    // Check owner
    let signer = &context.borrow().get_state().signer.get_transaction_signer();
    let minter_address = signer
        .get_address()
        .await
        .map_err(|e| Error::from(format!("failed to get address: {e}")))?;
    let data = MINTER_CANISTER_ADDRESS
        .encode_input(&[])
        .map_err(|e| Error::from(format!("failed to encode function arguments: {e}")))?;
    let call_result = evm
        .eth_call(
            Some(minter_address.clone()),
            Some(bft_address),
            None,
            DEFAULT_TX_GAS_LIMIT,
            None,
            Some(data.into()),
            context,
        )
        .await?;
    let call_result = hex::decode(call_result.trim_start_matches("0x"))
        .map_err(|e| Error::from(format!("failed to decode call result: {e}")))?;
    if let &[Token::Address(contract_minter_address)] = MINTER_CANISTER_ADDRESS
        .decode_output(&call_result)
        .map_err(|e| Error::from(format!("failed to decode call result: {e}")))?
        .as_slice()
    {
        if H160::from(contract_minter_address) != minter_address {
            Err(Error::InvalidBftBridgeContract)
        } else {
            Ok(())
        }
    } else {
        Err(Error::from("invalid call result".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use candid::Principal;
    use did::{codec, H160};
    use eth_signer::sign_strategy::{SigningStrategy, TransactionSigner};
    use ethers_core::utils::keccak256;
    use ic_exports::ic_kit::MockContext;
    use ic_stable_structures::Storable;
    use minter_did::id256::Id256;
    use minter_did::order::{fit_str_to_array, MintOrder, SignedMintOrder};

    #[tokio::test]
    async fn signed_mint_order_encoding_roundtrip() {
        MockContext::new().inject();
        let signer = SigningStrategy::Local {
            private_key: [42; 32],
        }
        .make_signer(256)
        .unwrap();

        let order = MintOrder {
            amount: 42u64.into(),
            sender: (&Principal::management_canister()).into(),
            src_token: Id256::from_evm_address(&H160::from_slice(&[12; 20]), 512),
            dst_token: H160::from_slice(&[13; 20]),
            recipient: H160::from_slice(&[24; 20]),
            nonce: 128,
            sender_chain_id: 200,
            recipient_chain_id: 256,
            name: fit_str_to_array("name"),
            symbol: fit_str_to_array("symbol"),
            decimals: 18,
        };

        let encoded = order.encode_and_sign(&signer).await.unwrap();
        let (decoded_order, signature) = MintOrder::decode_signed(&encoded).unwrap();
        assert_eq!(order, decoded_order);

        let digest = keccak256(&encoded[..MintOrder::ENCODED_DATA_SIZE]);
        ethers_core::types::Signature::from(signature)
            .verify(digest, signer.get_address().await.unwrap())
            .unwrap();
    }

    #[test]
    fn test_signed_mint_order_candid_encoding() {
        let order = SignedMintOrder([42; MintOrder::SIGNED_ENCODED_DATA_SIZE]);
        let decoded = codec::decode::<SignedMintOrder>(&codec::encode(&order));
        assert_eq!(decoded, order);
    }

    #[test]
    fn test_signed_mint_order_candid_storable() {
        let order = SignedMintOrder([42; MintOrder::SIGNED_ENCODED_DATA_SIZE]);
        let decoded = SignedMintOrder::from_bytes(order.to_bytes());
        assert_eq!(decoded, order);
    }

    #[test]
    fn test_fit_str_to_array() {
        // The '🌸' is four byte symbol: 0xf09f8cb8;

        let a: [u8; 4] = fit_str_to_array("🌸");
        assert_eq!(a, [0xf0, 0x9f, 0x8c, 0xb8]);

        let a: [u8; 4] = fit_str_to_array("0🌸");
        assert_eq!(a, [0x30, 0x00, 0x00, 0x00]);
        let a: [u8; 4] = fit_str_to_array("00🌸");
        assert_eq!(a, [0x30, 0x30, 0x00, 0x00]);
        let a: [u8; 4] = fit_str_to_array("000🌸");
        assert_eq!(a, [0x30, 0x30, 0x30, 0x00]);
        let a: [u8; 4] = fit_str_to_array("0000🌸");
        assert_eq!(a, [0x30, 0x30, 0x30, 0x30]);
        let a: [u8; 4] = fit_str_to_array("00000🌸");
        assert_eq!(a, [0x30, 0x30, 0x30, 0x30]);

        let a: [u8; 4] = fit_str_to_array("");
        assert_eq!(a, [0; 4]);
    }
}
