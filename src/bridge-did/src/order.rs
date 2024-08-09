use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use candid::CandidType;
use did::transaction::Signature;
use did::{H160, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::utils::keccak256;
use ic_stable_structures::{Bound, Storable};
use serde::de::Visitor;
use serde::{Deserialize, Serialize};

use crate::error::{BftResult, Error};
use crate::id256::Id256;

/// Data which should be signed and provided to the `BftBridge.mint()` call
/// to perform mint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct MintOrder {
    /// Amount of tokens to mint.
    pub amount: U256,

    /// Identifier of the user who performs the mint.
    pub sender: Id256,

    /// Identifier of the source token.
    pub src_token: Id256,

    /// Address of the receiver of the mint.
    pub recipient: H160,

    /// Destination token for which mint operation will be performed.
    pub dst_token: H160,

    /// Value to prevent double spending.
    pub nonce: u32,

    /// ChainId of EVM on which user will send tokens to bridge.
    pub sender_chain_id: u32,

    /// ChainId of EVM on which will send tokens to user.
    /// Used to prevent several cross-chain mints with the same order.
    pub recipient_chain_id: u32,

    /// Name of the token.
    pub name: [u8; 32],

    /// Symbol of the token.
    pub symbol: [u8; 16],

    /// Decimals of the token.
    pub decimals: u8,

    /// Mint operation should approve tokens, using this address as a spender.
    pub approve_spender: H160,

    /// Mint operation should approve this amount of tokens.
    pub approve_amount: U256,

    /// Address of wallet from which fee will be charged.
    pub fee_payer: H160,
}

impl MintOrder {
    pub const ENCODED_DATA_SIZE: usize = 269;
    pub const SIGNED_ENCODED_DATA_SIZE: usize = Self::ENCODED_DATA_SIZE + 65;

    /// Encodes order data and signs it.
    /// Encoded data layout:
    /// ```ignore
    /// [
    ///     0..32 bytes of amount,                  }
    ///     32..64 bytes of sender,                 }
    ///     64..96 bytes of src_token,              }
    ///     96..116 bytes of recipient,             }
    ///     116..136 bytes of dst_token,            }
    ///     136..140 bytes of nonce,                } => signed data
    ///     140..144 bytes of sender_chain_id,      }
    ///     144..148 bytes of recipient_chain_id,   }
    ///     148..180 bytes of name,                 }
    ///     180..196 bytes of symbol,               }
    ///     196..197 bytes of decimals,             }
    ///     197..217 bytes of approve_address,      }
    ///     217..249 bytes of approve_amount,       }
    ///     249..269 bytes of fee_payer,            }
    ///     269..334 bytes of signature (r - 32 bytes, s - 32 bytes, v - 1 byte)
    /// ]
    /// ```
    ///
    /// All integers encoded in big-endian format.
    /// Signature signs KECCAK hash of the signed data.
    pub async fn encode_and_sign(
        &self,
        signer: &impl TransactionSigner,
    ) -> BftResult<SignedMintOrder> {
        let mut buf = [0; Self::SIGNED_ENCODED_DATA_SIZE];

        buf[..32].copy_from_slice(&self.amount.to_big_endian());
        buf[32..64].copy_from_slice(self.sender.0.as_slice());
        buf[64..96].copy_from_slice(self.src_token.0.as_slice());
        buf[96..116].copy_from_slice(self.recipient.0.as_bytes());
        buf[116..136].copy_from_slice(self.dst_token.0.as_bytes());
        buf[136..140].copy_from_slice(&self.nonce.to_be_bytes());
        buf[140..144].copy_from_slice(&self.sender_chain_id.to_be_bytes());
        buf[144..148].copy_from_slice(&self.recipient_chain_id.to_be_bytes());
        buf[148..180].copy_from_slice(&self.name);
        buf[180..196].copy_from_slice(&self.symbol);
        buf[196] = self.decimals;
        buf[197..217].copy_from_slice(self.approve_spender.0.as_bytes());
        buf[217..249].copy_from_slice(&self.approve_amount.to_big_endian());
        buf[249..269].copy_from_slice(self.fee_payer.0.as_bytes());

        let digest = keccak256(&buf[..Self::ENCODED_DATA_SIZE]);

        // Sign fields data hash.
        let signature = signer
            .sign_digest(digest)
            .await
            .map_err(|e| Error::Signing(format!("failed to sign MintOrder: {e}")))?;

        // Add signature to the data.
        let signature_bytes: [u8; 65] = ethers_core::types::Signature::from(signature).into();
        buf[Self::ENCODED_DATA_SIZE..].copy_from_slice(&signature_bytes);

        Ok(SignedMintOrder(buf))
    }

    /// Decode Self from bytes.
    pub fn decode_data(data: &[u8]) -> Option<Self> {
        if data.len() < Self::ENCODED_DATA_SIZE {
            return None;
        }

        let amount = U256::from_big_endian(&data[..32]);
        let sender = data[32..64].try_into().unwrap(); // exactly 32 bytes, as expected
        let src_token = data[64..96].try_into().unwrap(); // exactly 32 bytes, as expected
        let recipient = H160::from_slice(&data[96..116]);
        let dst_token = H160::from_slice(&data[116..136]);
        let nonce = u32::from_be_bytes(data[136..140].try_into().unwrap()); // exactly 4 bytes, as expected
        let sender_chain_id = u32::from_be_bytes(data[140..144].try_into().unwrap()); // exactly 4 bytes, as expected
        let recipient_chain_id = u32::from_be_bytes(data[144..148].try_into().unwrap()); // exactly 4 bytes, as expected
        let name = data[148..180].try_into().unwrap(); // exactly 32 bytes, as expected
        let symbol = data[180..196].try_into().unwrap(); // exactly 16 bytes, as expected
        let decimals = data[196];
        let approve_spender = H160::from_slice(&data[197..217]);
        let approve_amount = U256::from_big_endian(&data[217..249]);
        let fee_payer = H160::from_slice(&data[249..269]);

        Some(Self {
            amount,
            sender,
            src_token,
            recipient,
            dst_token,
            nonce,
            sender_chain_id,
            recipient_chain_id,
            name,
            symbol,
            decimals,
            approve_spender,
            approve_amount,
            fee_payer,
        })
    }

    /// Decode Self from bytes.
    pub fn decode_signed(data: &SignedMintOrder) -> Option<(Self, Signature)> {
        if data.len() < Self::SIGNED_ENCODED_DATA_SIZE {
            return None;
        }

        let decoded_data = Self::decode_data(data.as_ref())?;
        let signature =
            ethers_core::types::Signature::try_from(&data[Self::ENCODED_DATA_SIZE..][..65])
                .ok()?
                .into();

        Some((decoded_data, signature))
    }
}

pub fn fit_str_to_array<const SIZE: usize>(s: &str) -> [u8; SIZE] {
    let mut size = SIZE.min(s.len());
    while !s.is_char_boundary(size) {
        size -= 1;
    }

    let mut buf = [0; SIZE];
    buf[..size].copy_from_slice(&s.as_bytes()[..size]);
    buf
}

/// New type for the SignedMintOrder.
/// Allows to implement `Deserialize + CandidType` traits.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SignedMintOrder(pub [u8; MintOrder::SIGNED_ENCODED_DATA_SIZE]);

/// Visitor for `SignedMintOrder` objects deserialization.
struct SignedMintOrderVisitor;

impl<'v> Visitor<'v> for SignedMintOrderVisitor {
    type Value = SignedMintOrder;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            formatter,
            "blob of size {}",
            MintOrder::SIGNED_ENCODED_DATA_SIZE
        )
    }

    fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(SignedMintOrder(
            v.try_into()
                .map_err(|_| E::invalid_length(v.len(), &Self))?,
        ))
    }
}

impl<'de> Deserialize<'de> for SignedMintOrder {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(SignedMintOrderVisitor)
    }
}

impl Serialize for SignedMintOrder {
    fn serialize<S>(&self, serializer: S) -> std::prelude::v1::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl CandidType for SignedMintOrder {
    fn _ty() -> candid::types::Type {
        candid::types::Type(Rc::new(candid::types::TypeInner::Vec(candid::types::Type(
            Rc::new(candid::types::TypeInner::Nat8),
        ))))
    }

    fn idl_serialize<S>(&self, serializer: S) -> std::result::Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        serializer.serialize_blob(&self.0)
    }
}

impl Deref for SignedMintOrder {
    type Target = [u8; MintOrder::SIGNED_ENCODED_DATA_SIZE];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SignedMintOrder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Storable for SignedMintOrder {
    const BOUND: Bound = Bound::Bounded {
        max_size: MintOrder::SIGNED_ENCODED_DATA_SIZE as _,
        is_fixed_size: true,
    };

    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.to_bytes()
    }

    fn from_bytes(bytes: std::borrow::Cow<'_, [u8]>) -> Self {
        Self(<[u8; MintOrder::SIGNED_ENCODED_DATA_SIZE]>::from_bytes(
            bytes,
        ))
    }
}

impl SignedMintOrder {
    /// Returns mint amount.
    pub fn get_amount(&self) -> U256 {
        U256::from_big_endian(&self.0[..32])
    }

    /// Returns sender ID.
    pub fn get_sender_id(&self) -> Id256 {
        self.0[32..64].try_into().unwrap() // exactly 32 bytes, as expected
    }

    /// Returns source token.
    pub fn get_src_token_id(&self) -> Id256 {
        self.0[64..96].try_into().unwrap() // exactly 32 bytes, as expected
    }

    /// Returns recipient address.
    pub fn get_recipient(&self) -> H160 {
        H160::from_slice(&self.0[96..116])
    }

    /// Returns dst_token address.
    pub fn get_dst_token(&self) -> H160 {
        H160::from_slice(&self.0[116..136])
    }

    /// Returns nonce.
    pub fn get_nonce(&self) -> u32 {
        u32::from_be_bytes(self.0[136..140].try_into().unwrap()) // exactly 4 bytes, as expected
    }

    /// Returns sender chain ID.
    pub fn get_sender_chain_id(&self) -> u32 {
        u32::from_be_bytes(self.0[140..144].try_into().unwrap()) // exactly 4 bytes, as expected
    }

    /// Returns recipient chain ID.
    pub fn get_recipient_chain_id(&self) -> u32 {
        u32::from_be_bytes(self.0[144..148].try_into().unwrap()) // exactly 4 bytes, as expected
    }

    /// Returns token name.
    pub fn get_token_name(&self) -> [u8; 32] {
        self.0[148..180].try_into().unwrap() // exactly 32 bytes, as expected
    }

    /// Returns token symbol.
    pub fn get_token_symbol(&self) -> [u8; 16] {
        self.0[180..196].try_into().unwrap() // exactly 16 bytes, as expected
    }

    /// Returns token decimals.
    pub fn get_token_decimals(&self) -> u8 {
        self.0[196]
    }

    /// Returns approve spender.
    pub fn get_approve_spender(&self) -> H160 {
        H160::from_slice(&self.0[197..217])
    }

    /// Returns approve amount.
    pub fn get_approve_amount(&self) -> U256 {
        U256::from_big_endian(&self.0[217..249])
    }

    /// Returns fee payer.
    pub fn get_fee_payer(&self) -> H160 {
        H160::from_slice(&self.0[249..269])
    }
}

#[cfg(test)]
mod tests {
    use did::{H160, U256};
    use eth_signer::sign_strategy::SigningStrategy;

    use super::MintOrder;
    use crate::id256::Id256;

    #[tokio::test]
    async fn signed_mint_order_getters() {
        let order = MintOrder {
            amount: U256::one(),
            sender: Id256::from_evm_address(&H160::from_slice(&[1; 20]), 1),
            src_token: Id256::from_evm_address(&H160::from_slice(&[2; 20]), 2),
            recipient: H160::from_slice(&[3; 20]),
            dst_token: H160::from_slice(&[4; 20]),
            nonce: 42,
            sender_chain_id: 43,
            recipient_chain_id: 44,
            name: [45; 32],
            symbol: [46; 16],
            decimals: 47,
            approve_spender: H160::from_slice(&[5; 20]),
            approve_amount: 48u64.into(),
            fee_payer: H160::from_slice(&[6; 20]),
        };

        let signer = SigningStrategy::Local {
            private_key: [42; 32],
        }
        .make_signer(0)
        .unwrap();
        let signed_order = order.encode_and_sign(&signer).await.unwrap();

        assert_eq!(order.amount, signed_order.get_amount());
        assert_eq!(order.sender, signed_order.get_sender_id());
        assert_eq!(order.src_token, signed_order.get_src_token_id());
        assert_eq!(order.recipient, signed_order.get_recipient());
        assert_eq!(order.dst_token, signed_order.get_dst_token());
        assert_eq!(order.nonce, signed_order.get_nonce());
        assert_eq!(order.sender_chain_id, signed_order.get_sender_chain_id());
        assert_eq!(
            order.recipient_chain_id,
            signed_order.get_recipient_chain_id()
        );
        assert_eq!(order.name, signed_order.get_token_name());
        assert_eq!(order.symbol, signed_order.get_token_symbol());
        assert_eq!(order.decimals, signed_order.get_token_decimals());
        assert_eq!(order.approve_spender, signed_order.get_approve_spender());
        assert_eq!(order.approve_amount, signed_order.get_approve_amount());
        assert_eq!(order.fee_payer, signed_order.get_fee_payer());
    }
}
