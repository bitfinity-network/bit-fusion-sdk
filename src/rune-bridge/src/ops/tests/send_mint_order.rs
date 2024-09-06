use bridge_canister::bridge::OperationContext;
use bridge_did::error::{BftResult, Error};
use bridge_did::order::SignedMintOrder;
use bridge_utils::evm_bridge::EvmParams;
use bridge_utils::evm_link::EvmLink;
use did::{H160, H256};
use eth_signer::sign_strategy::TransactionSigner;
use snapbox::{assert_data_eq, str};

use crate::ops::tests::sign_mint_order::TestSigner;
use crate::ops::tests::test_signed_order;
use crate::ops::{RuneBridgeDepositOp, RuneBridgeOp};

struct TestOperationContext {
    result: BftResult<H256>,
}

impl TestOperationContext {
    fn err(err: Error) -> Self {
        Self { result: Err(err) }
    }

    fn hash(seed: u8) -> H256 {
        H256::from_slice(&[seed; 32])
    }

    fn ok(seed: u8) -> Self {
        Self {
            result: Ok(Self::hash(seed)),
        }
    }
}

impl OperationContext for TestOperationContext {
    fn get_evm_link(&self) -> EvmLink {
        unimplemented!()
    }

    fn get_bridge_contract_address(&self) -> BftResult<H160> {
        unimplemented!()
    }

    fn get_evm_params(&self) -> BftResult<EvmParams> {
        unimplemented!()
    }

    fn get_signer(&self) -> BftResult<impl TransactionSigner> {
        Ok(TestSigner::ok())
    }

    async fn send_mint_transaction(&self, _order: &SignedMintOrder) -> BftResult<H256> {
        self.result.clone()
    }
}

#[tokio::test]
async fn returns_error_if_fails_to_send() {
    let ctx = TestOperationContext::err(Error::EvmRequestFailed("too many hedgehogs".to_string()));
    let err = RuneBridgeOp::send_mint_order(&ctx, test_signed_order().await)
        .await
        .expect_err("operation succeeded unexpectedly");

    assert_data_eq!(
        err.to_string(),
        str!["EVM request failed: too many hedgehogs"]
    );
}

#[tokio::test]
async fn return_correct_operation_if_success() {
    const SEED: u8 = 47;
    let ctx = TestOperationContext::ok(SEED);
    let signed_order = test_signed_order().await;
    let op = RuneBridgeOp::send_mint_order(&ctx, signed_order)
        .await
        .expect("operation failed");

    let RuneBridgeOp::Deposit(RuneBridgeDepositOp::ConfirmMintOrder { order, tx_id }) = op else {
        panic!("Unexpected operation: {op:?}");
    };

    assert_eq!(order, signed_order);
    assert_eq!(tx_id, TestOperationContext::hash(SEED));
}
