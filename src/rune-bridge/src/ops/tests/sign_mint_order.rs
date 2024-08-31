use async_trait::async_trait;
use bridge_did::id256::Id256;
use bridge_did::order::MintOrder;
use did::transaction::Signature;
use did::H160;
use eth_signer::sign_strategy::{
    TransactionSigner, TransactionSignerError, TransactionSignerResult,
};
use eth_signer::WalletError;
use ethers_core::types::transaction::eip2718::TypedTransaction;
use snapbox::{assert_data_eq, str};

use crate::ops::tests::sender;
use crate::ops::RuneBridgeOp;

struct TestSigner {
    signing_error: Option<String>,
}

impl TestSigner {
    fn ok() -> Self {
        Self {
            signing_error: None,
        }
    }

    fn with_err(err: &str) -> Self {
        Self {
            signing_error: Some(err.to_string()),
        }
    }
}

#[async_trait(?Send)]
impl TransactionSigner for TestSigner {
    async fn get_address(&self) -> TransactionSignerResult<H160> {
        Ok(H160::from_slice(&[1; 160]))
    }

    async fn sign_transaction(
        &self,
        _transaction: &TypedTransaction,
    ) -> TransactionSignerResult<Signature> {
        match &self.signing_error {
            Some(message) => Err(TransactionSignerError::WalletError(
                WalletError::Eip712Error(message.clone()),
            )),
            None => Ok(Signature::default()),
        }
    }

    async fn sign_digest(&self, _digest: [u8; 32]) -> TransactionSignerResult<Signature> {
        match &self.signing_error {
            Some(message) => Err(TransactionSignerError::WalletError(
                WalletError::Eip712Error(message.clone()),
            )),
            None => Ok(Signature::default()),
        }
    }

    async fn get_public_key(&self) -> TransactionSignerResult<Vec<u8>> {
        Ok([1; 20].to_vec())
    }
}

fn test_mint_order() -> MintOrder {
    MintOrder {
        amount: Default::default(),
        sender: Id256::from_evm_address(&H160::from_slice(&[1; 20]), 1),
        src_token: Id256::from_evm_address(&H160::from_slice(&[2; 20]), 1),
        recipient: Default::default(),
        dst_token: Default::default(),
        nonce: 0,
        sender_chain_id: 0,
        recipient_chain_id: 0,
        name: [1; 32],
        symbol: [1; 16],
        decimals: 0,
        approve_spender: Default::default(),
        approve_amount: Default::default(),
        fee_payer: Default::default(),
    }
}

#[tokio::test]
async fn returns_error_if_cannot_sign() {
    let signer = TestSigner::with_err("something strange".into());
    let mint_order = test_mint_order();
    let err = RuneBridgeOp::sign_mint_order(&signer, 3, sender(), mint_order)
        .await
        .expect_err("signing was unexpectedly successful");

    assert_data_eq!(err.to_string(), str![[r#"signer failure: failed to sign MintOrder: wallet error: error encoding eip712 struct: "something strange""#]]);
}

#[tokio::test]
async fn returns_correct_operation_and_sets_nonce() {
    let signer = TestSigner::ok();
    let mint_order = test_mint_order();
    const NONCE: u32 = 42;
    let op = RuneBridgeOp::sign_mint_order(&signer, NONCE, sender(), mint_order)
        .await
        .expect("signing failed unexpectedly");

    let RuneBridgeOp::SendMintOrder { dst_address, order } = op else {
        panic!("Unexpected resulting operation: {op:?}");
    };

    assert_eq!(dst_address, sender());
    assert_eq!(order.get_nonce(), NONCE);
}
