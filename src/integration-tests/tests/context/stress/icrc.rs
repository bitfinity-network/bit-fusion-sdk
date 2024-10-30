use std::sync::atomic::{AtomicU32, Ordering};

use alloy_sol_types::SolCall;
use bridge_client::BridgeCanisterClient;
use bridge_did::id256::Id256;
use bridge_did::operation_log::Memo;
use bridge_did::operations::IcrcBridgeOp;
use bridge_did::reason::Icrc2Burn;
use bridge_utils::{evm_link, BFTBridge};
use candid::{Encode, Nat, Principal};
use did::error::{EvmError, TransactionPoolError};
use did::{H160, U256};
use eth_signer::Signer;
use ic_exports::icrc_types::icrc1_ledger::LedgerArgument;
use icrc_client::account::Account;
use icrc_client::allowance::AllowanceArgs;
use icrc_client::approve::ApproveArgs;
use icrc_client::transfer::TransferArg;

use super::{BaseTokens, BurnInfo, OwnedWallet, StressTestConfig, StressTestState, User};
use crate::context::{icrc_canister_default_init_args, CanisterType, TestContext};
use crate::utils::error::{Result, TestError};

static USER_COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct IcrcBaseTokens<Ctx> {
    ctx: Ctx,
    tokens: Vec<Principal>,
}

impl<Ctx: TestContext> IcrcBaseTokens<Ctx> {
    async fn init(ctx: Ctx, base_tokens_number: usize) -> Result<Self> {
        println!("Creating icrc token canisters");
        let mut tokens = Vec::with_capacity(base_tokens_number);
        for token_idx in 0..base_tokens_number {
            let icrc_principal = Self::init_icrc_token_canister(&ctx, token_idx).await?;
            tokens.push(icrc_principal);
        }

        println!("Icrc token canisters created");

        Ok(Self { ctx, tokens })
    }

    async fn init_icrc_token_canister(ctx: &Ctx, token_idx: usize) -> Result<Principal> {
        let token = ctx.create_canister().await?;

        let init_balances = vec![];
        let init_data = icrc_canister_default_init_args(
            ctx.admin(),
            &format!("Tkn#{token_idx}"),
            init_balances,
        );
        let wasm = CanisterType::Icrc1Ledger.default_canister_wasm().await;
        ctx.install_canister(token, wasm, (LedgerArgument::Init(init_data),))
            .await
            .unwrap();

        Ok(token)
    }
}

impl<Ctx: TestContext + Send + Sync> BaseTokens for IcrcBaseTokens<Ctx> {
    type TokenId = Principal;
    type UserId = String;

    fn ctx(&self) -> &(impl TestContext + Send + Sync) {
        &self.ctx
    }

    fn ids(&self) -> &[Self::TokenId] {
        &self.tokens
    }

    fn user_id256(&self, user_id: Self::UserId) -> Id256 {
        let principal = self.ctx.principal_by_caller_name(&user_id);
        (&principal).into()
    }

    fn token_id256(&self, token_id: Self::TokenId) -> Id256 {
        (&token_id).into()
    }

    async fn bridge_canister_evm_address(&self) -> Result<H160> {
        let client = self.ctx.icrc_bridge_client(self.ctx.admin_name());
        let address = client.get_bridge_canister_evm_address().await??;
        Ok(address)
    }

    async fn new_user(&self, _wrapped_wallet: &OwnedWallet) -> Result<Self::UserId> {
        Ok(format!(
            "icrc {}",
            USER_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    async fn mint(&self, token_idx: usize, to: &Self::UserId, amount: U256) -> Result<()> {
        let token_principal = self.tokens[token_idx];
        let client = self
            .ctx
            .icrc_token_client(token_principal, self.ctx.admin_name());
        let to_principal = self.ctx.principal_by_caller_name(to);
        let amount: Nat = (&amount).into();
        let transfer_args = TransferArg {
            from_subaccount: None,
            to: to_principal.into(),
            fee: None,
            created_at_time: None,
            memo: None,
            amount: amount.clone(),
        };
        client.icrc1_transfer(transfer_args).await??;

        let balance = client.icrc1_balance_of(to_principal.into()).await.unwrap();
        assert_eq!(amount, balance);

        Ok(())
    }

    async fn balance_of(&self, token_idx: usize, user: &Self::UserId) -> Result<U256> {
        let token_principal = self.tokens[token_idx];
        let client = self
            .ctx
            .icrc_token_client(token_principal, self.ctx.admin_name());

        let user_principal = self.ctx.principal_by_caller_name(user);

        let balance = client.icrc1_balance_of(user_principal.into()).await?;
        Ok((&balance).try_into().expect("balance is too big"))
    }

    async fn deposit(
        &self,
        to_user: &User<Self::UserId>,
        info: &BurnInfo<Self::UserId>,
    ) -> Result<U256> {
        let token_principal = self.tokens[info.base_token_idx];
        let client = self.ctx.icrc_token_client(token_principal, &info.from);

        let to = to_user.wallet.address();
        let subaccount = Some(evm_link::address_to_icrc_subaccount(&to));
        let minter_canister = Account {
            owner: self.ctx.canisters().icrc2_bridge(),
            subaccount,
        };

        let sender = self.ctx.principal_by_caller_name(&info.from);
        let args = AllowanceArgs {
            account: sender.into(),
            spender: minter_canister,
        };
        let allowance = match client.icrc2_allowance(args).await {
            Ok(a) => a,
            Err(e) => return Err(TestError::from(e)),
        };
        if allowance.allowance == 0u64 {
            let approve_args = ApproveArgs {
                from_subaccount: None,
                spender: minter_canister,
                amount: u64::MAX.into(),
                expected_allowance: None,
                expires_at: None,
                fee: None,
                memo: None,
                created_at_time: None,
            };

            client.icrc2_approve(approve_args).await?.unwrap();
        }

        let reason = Icrc2Burn {
            sender,
            amount: info.amount.clone(),
            from_subaccount: None,
            icrc2_token_principal: token_principal,
            erc20_token_address: info.wrapped_token.clone(),
            recipient_address: to.into(),
            fee_payer: Some(to.into()),
            approve_after_mint: None,
        };

        let encoded_reason = Encode!(&reason).unwrap();

        let input = BFTBridge::notifyMinterCall {
            notificationType: Default::default(),
            userData: encoded_reason.into(),
            memo: info.memo.into(),
        }
        .abi_encode();

        loop {
            let call_result = self
                .ctx
                .call_contract_without_waiting(
                    &to_user.wallet,
                    &info.bridge,
                    input.clone(),
                    0,
                    Some(to_user.next_nonce()),
                )
                .await;

            // Retry on invalid nonce or alerady exits.
            match call_result {
                Err(TestError::Evm(EvmError::TransactionPool(
                    TransactionPoolError::InvalidNonce { .. },
                )))
                | Err(TestError::Evm(EvmError::TransactionPool(
                    TransactionPoolError::TransactionAlreadyExists,
                ))) => continue,
                _ => (),
            }

            call_result?;
            break;
        }

        Ok(info.amount.clone())
    }

    async fn set_bft_bridge_contract_address(&self, bft_bridge: &H160) -> Result<()> {
        self.ctx
            .icrc_bridge_client(self.ctx.admin_name())
            .set_bft_bridge_contract(bft_bridge)
            .await?;

        Ok(())
    }

    async fn is_operation_complete(&self, address: H160, memo: Memo) -> Result<bool> {
        let op_info = self
            .ctx
            .icrc_bridge_client(self.ctx.admin_name())
            .get_operation_by_memo_and_user(memo, &address)
            .await?;

        let op = match op_info {
            Some((_, op)) => op,
            None => {
                return Err(TestError::Generic("operation not found".into()));
            }
        };

        let is_complete = matches!(
            op,
            IcrcBridgeOp::WrappedTokenMintConfirmed(_) | IcrcBridgeOp::IcrcMintConfirmed { .. }
        );
        Ok(is_complete)
    }
}

/// Run stress test with the given TestContext implementation.
pub async fn stress_test_icrc_bridge_with_ctx<T>(
    ctx: T,
    base_tokens_number: usize,
    config: StressTestConfig,
) where
    T: TestContext + Send + Sync,
{
    let base_tokens = IcrcBaseTokens::init(ctx, base_tokens_number).await.unwrap();
    let icrc_stress_test_stats = StressTestState::run(base_tokens, config).await.unwrap();

    dbg!(&icrc_stress_test_stats);

    assert_eq!(icrc_stress_test_stats.failed_roundtrips, 0);
}
