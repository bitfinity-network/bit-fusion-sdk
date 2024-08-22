use std::sync::atomic::{AtomicU32, Ordering};

use alloy_sol_types::SolCall;
use bridge_client::BridgeCanisterClient;
use bridge_did::reason::Icrc2Burn;
use bridge_utils::{evm_link, BFTBridge};
use candid::{Encode, Principal};
use did::{H160, U256};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use icrc_client::account::Account;
use icrc_client::approve::ApproveArgs;
use icrc_client::transfer::TransferArg;

use crate::context::TestContext;
use crate::dfx_tests::ADMIN;
use crate::utils::error::Result;

use super::{BaseTokens, BurnInfo};

static USER_COUNTER: AtomicU32 = AtomicU32::new(256);

pub struct IcrcBaseTokens<Ctx> {
    ctx: Ctx,
    bridge_canister: Principal,
    tokens: Vec<Principal>,
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
            owner: self.bridge_canister,
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
