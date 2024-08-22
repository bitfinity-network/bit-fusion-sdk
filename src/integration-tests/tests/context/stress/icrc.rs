use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use bridge_client::BridgeCanisterClient;
use candid::Principal;
use did::{H160, U256};
use icrc_client::transfer::TransferArg;
use tokio::sync::Mutex;

use crate::context::TestContext;
use crate::dfx_tests::ADMIN;
use crate::utils::error::Result;

use super::BaseTokens;

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

    async fn bridge_canister_evm_address(&self) -> Result<H160> {
        let client = self.ctx.icrc_bridge_client(ADMIN);
        let address = client.get_bridge_canister_evm_address().await??;
        Ok(address)
    }

    async fn new_user(&self) -> Result<Self::UserId> {
         format!("icrc {}", USER_COUNTER.fetch_add(1, Ordering::Relaxed))
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

    async fn deposit(&self, info: &super::BurnInfo<Self::UserId>) -> Result<U256> {
        let client = self.ctx.icrc_token_1_client(caller);

        let subaccount = Some(address_to_icrc_subaccount(&recipient.0));
        let minter_canister = Account {
            owner: self.canisters().icrc2_bridge(),
            subaccount,
        };

        let approve_args = ApproveArgs {
            from_subaccount: None,
            spender: minter_canister,
            amount: amount.into(),
            expected_allowance: None,
            expires_at: None,
            fee: None,
            memo: None,
            created_at_time: None,
        };

        client.icrc2_approve(approve_args).await?.unwrap();
        Ok(())

        let recipient_address = H160::from(wallet.address());
        self.approve_icrc2_burn(
            caller,
            &recipient_address,
            amount + ICRC1_TRANSFER_FEE as u128,
        )
        .await?;

        let reason = Icrc2Burn {
            sender: self.principal_by_caller_name(caller),
            amount: amount.into(),
            from_subaccount: None,
            icrc2_token_principal: self.canisters().token_1(),
            erc20_token_address: erc20_token_address.clone(),
            recipient_address,
            fee_payer,
            approve_after_mint,
        };

        let encoded_reason = Encode!(&reason).unwrap();

        let input = BFTBridge::notifyMinterCall {
            notificationType: Default::default(),
            userData: encoded_reason.into(),
        }
        .abi_encode();

        let _receipt = self
            .call_contract(wallet, bridge, input, 0)
            .await
            .map(|(_, receipt)| receipt)?;

        Ok(())
    }

    fn user_id256(&self, user_id: Self::UserId) -> bridge_did::id256::Id256 {
        todo!()
    }
}
