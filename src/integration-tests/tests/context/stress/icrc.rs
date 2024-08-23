use std::sync::atomic::{AtomicU32, Ordering};

use alloy_sol_types::SolCall;
use bridge_client::BridgeCanisterClient;
use bridge_did::reason::Icrc2Burn;
use bridge_utils::{evm_link, BFTBridge};
use candid::{Encode, Principal};
use did::{H160, U256};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use ic_exports::icrc_types::icrc1_ledger::LedgerArgument;
use icrc_client::account::Account;
use icrc_client::approve::ApproveArgs;
use icrc_client::transfer::TransferArg;

use crate::context::{icrc_canister_default_init_args, CanisterType, TestContext};
use crate::dfx_tests::ADMIN;
use crate::utils::error::Result;

use super::{BaseTokens, BurnInfo, StressTestConfig, StressTestState};

static USER_COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct IcrcBaseTokens<Ctx> {
    ctx: Ctx,
    tokens: Vec<Principal>,
}

impl<Ctx: TestContext> IcrcBaseTokens<Ctx> {
    async fn init(ctx: Ctx, base_tokens_number: usize) -> Result<Self> {
        let mut tokens = Vec::with_capacity(base_tokens_number);
        for token_idx in 0..base_tokens_number {
            let icrc_principal = Self::init_icrc_token_canister(&ctx, token_idx).await?;
            tokens.push(icrc_principal);
        }

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

    fn user_id256(&self, user_id: Self::UserId) -> bridge_did::id256::Id256 {
        let principal = self.ctx.principal_by_caller_name(&user_id);
        (&principal).into()
    }

    async fn bridge_canister_evm_address(&self) -> Result<H160> {
        let client = self.ctx.icrc_bridge_client(ADMIN);
        let address = client.get_bridge_canister_evm_address().await??;
        Ok(address)
    }

    async fn new_user(&self) -> Result<Self::UserId> {
        Ok(format!(
            "icrc {}",
            USER_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    async fn mint(&self, token_idx: usize, to: &Self::UserId, amount: U256) -> Result<()> {
        let token_principal = self.tokens[token_idx];
        let client = self.ctx.icrc_token_client(token_principal, ADMIN);
        let to_principal = self.ctx.principal_by_caller_name(to);
        let transfer_args = TransferArg {
            from_subaccount: None,
            to: to_principal.into(),
            fee: None,
            created_at_time: None,
            memo: None,
            amount: (&amount).into(),
        };
        client.icrc1_transfer(transfer_args).await??;
        Ok(())
    }

    async fn deposit(
        &self,
        to_wallet: &Wallet<'_, SigningKey>,
        info: &BurnInfo<Self::UserId>,
    ) -> Result<U256> {
        let token_principal = self.tokens[info.base_token_idx];
        let client = self.ctx.icrc_token_client(token_principal, &info.from);

        let to = to_wallet.address();
        let subaccount = Some(evm_link::address_to_icrc_subaccount(&to));
        let minter_canister = Account {
            owner: self.ctx.canisters().icrc2_bridge(),
            subaccount,
        };

        let approve_args = ApproveArgs {
            from_subaccount: None,
            spender: minter_canister,
            amount: (&info.amount).into(),
            expected_allowance: None,
            expires_at: None,
            fee: None,
            memo: None,
            created_at_time: None,
        };

        client.icrc2_approve(approve_args).await?.unwrap();

        let sender = self.ctx.principal_by_caller_name(&info.from);
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
        }
        .abi_encode();

        let _receipt = self
            .ctx
            .call_contract(to_wallet, &info.bridge, input, 0)
            .await
            .map(|(_, receipt)| receipt)?;

        Ok(info.amount.clone())
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

    assert!(icrc_stress_test_stats.failed_deposits == 0);
    assert!(icrc_stress_test_stats.failed_withdrawals == 0);
}
