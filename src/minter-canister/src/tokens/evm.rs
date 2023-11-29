use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use did::{H160, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::abi::Token;
use ic_exports::icrc_types::icrc1::account::Account;
use minter_contract_utils::bft_bridge_api::{self, GET_PENDING_BURN_INFO};
use minter_did::error::{Error, Result};
use minter_did::id256::Id256;
use minter_did::order::{self, MintOrder, SignedMintOrder};
use minter_did::reason::Icrc2Burn;

use super::bft_bridge::ValidBurn;
use super::{icrc1, TxHash};
use crate::constant::{DEFAULT_TX_GAS_LIMIT, IC_CHAIN_ID};
use crate::context::Context;
use crate::tokens::icrc2;

/// Structures that aggregates all EVM operations
#[derive(Default)]
pub struct EvmTokensService {}

impl EvmTokensService {
    pub async fn check_erc_20_burn(
        &self,
        user: &H160,
        operation_id: u32,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<ValidBurn<H160, Id256>> {
        let burn_info = self.query_burn_info(user, operation_id, context).await?;

        if burn_info.is_dummy() {
            return Err(Error::InvalidBurnOperation(
                "burn info not found for the given operation id".into(),
            ));
        }

        let chain_id = context.borrow().get_state().config.get_evmc_chain_id();

        Ok(ValidBurn {
            amount: burn_info.amount,
            sender: burn_info.sender,
            src_token: burn_info.from_erc_20,
            sender_chain_id: chain_id,
            recipient: burn_info.recipient_id,
            to_token: burn_info.to_token,
            recipient_chain_id: IC_CHAIN_ID,
            nonce: operation_id,
            name: burn_info.name,
            symbol: burn_info.symbol,
            decimals: burn_info.decimals,
        })
    }

    pub async fn check_erc_20_burn_finalized(
        &self,
        user: &H160,
        operation_id: u32,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<()> {
        let burn_info = self.query_burn_info(user, operation_id, context).await?;
        if !burn_info.is_dummy() {
            return Err(Error::InvalidBurnOperation(
                "burn info should be finished before ICRC-2 token transfer".into(),
            ));
        }

        Ok(())
    }

    /// Creates a signed mint order data for the given reason.
    pub async fn create_mint_order_for(
        &self,
        caller: Principal,
        reason: Icrc2Burn,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<SignedMintOrder> {
        let icrc2_token = reason.icrc2_token_principal;
        let token_info = icrc1::query_token_info_or_read_from_cache(icrc2_token)
            .await
            .ok_or(Error::InvalidBurnOperation(
                "failed to get token info".into(),
            ))?;

        let from = Account {
            owner: caller,
            subaccount: reason.from_subaccount,
        };

        // query BFT bridge for token address
        Self::get_erc20_address_from_bft_bridge(context, (&icrc2_token).into()).await?;

        icrc2::burn(icrc2_token, from, (&reason.amount).into(), true).await?;

        let nonce = context.borrow().mut_state().next_nonce();

        let name = order::fit_str_to_array(&token_info.name);
        let symbol = order::fit_str_to_array(&token_info.symbol);
        let chain_id = {
            let evmc = context.borrow().get_evm_canister();
            evmc.eth_chain_id(context).await?
        } as u32;

        let valid_burn: ValidBurn<Id256, H160> = ValidBurn {
            amount: reason.amount,
            sender: (&caller).into(),
            src_token: (&icrc2_token).into(),
            sender_chain_id: IC_CHAIN_ID,
            recipient: reason.recipient_address,
            to_token: Id256::no_to_address(),
            recipient_chain_id: chain_id,
            nonce,
            name,
            symbol,
            decimals: token_info.decimals,
        };

        let mint_order = MintOrder {
            amount: valid_burn.amount,
            sender: valid_burn.sender,
            src_token: valid_burn.src_token,
            recipient: valid_burn.recipient,
            dst_token: valid_burn.to_token,
            nonce: valid_burn.nonce,
            sender_chain_id: valid_burn.sender_chain_id,
            recipient_chain_id: valid_burn.recipient_chain_id,
            name: valid_burn.name,
            symbol: valid_burn.symbol,
            decimals: valid_burn.decimals,
        };

        let signed_mint_order = sign_mint_order(mint_order, context).await?;

        // Store mint order data locally if request for the order can not be repeated.
        context.borrow_mut().mut_state().mint_orders.insert(
            valid_burn.sender,
            valid_burn.src_token,
            reason.operation_id,
            &signed_mint_order,
        );

        Ok(signed_mint_order)
    }

    pub async fn mint_native_tokens(
        &self,
        caller: Principal,
        reason: Icrc2Burn,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<TxHash> {
        let signed_order = self.create_mint_order_for(caller, reason, context).await?;
        let order = MintOrder::decode_data(&signed_order.0).ok_or(Error::Internal(
            "failed to decode mint order which is just created".into(),
        ))?;

        // ensure it is native token
        if order.recipient_chain_id != context.borrow().get_state().config.get_evmc_chain_id() {
            return Err(Error::InvalidBurnOperation(
                "invalid destination chain id".to_string(),
            ));
        }
        let bridge_contract = context
            .borrow()
            .get_state()
            .config
            .get_bft_bridge_contract()
            .ok_or(Error::BftBridgeDoesNotExist)?;
        if order.dst_token != Id256::native_address() {
            return Err(Error::InvalidBurnOperation(
                "invalid destination contract address".to_string(),
            ));
        }

        let signed_order = sign_mint_order(order, context).await?;

        let call_data = bft_bridge_api::MINT
            .encode_input(&[Token::Bytes(signed_order.to_vec())])
            .map_err(|e| Error::from(format!("failed to encode call data: {e}")))?;
        let evm = context.borrow().get_evm_canister();
        evm.transact(U256::zero(), bridge_contract, call_data, context)
            .await
    }

    /// Get ERC20 address from BFT bridge given the ICRC2 token
    async fn get_erc20_address_from_bft_bridge(
        context: &Rc<RefCell<dyn Context>>,
        icrc2_token: Id256,
    ) -> Result<H160> {
        let signer;
        {
            let ctx = context.borrow();
            signer = ctx.get_state().signer.clone();
        }

        let contract_minter_address = signer
            .get_transaction_signer()
            .get_address()
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let bft_bridge = context
            .borrow()
            .get_state()
            .config
            .get_bft_bridge_contract()
            .ok_or(Error::BftBridgeDoesNotExist)?;

        let data = bft_bridge_api::GET_WRAPPED_TOKEN
            .encode_input(&[Token::FixedBytes(icrc2_token.0.to_vec())])
            .map_err(|e| Error::from(format!("failed to encode function arguments: {e}")))?;
        let evm = context.borrow().get_evm_canister();
        let call_result = evm
            .eth_call(
                Some(contract_minter_address),
                Some(bft_bridge),
                None,
                DEFAULT_TX_GAS_LIMIT,
                None,
                Some(data.into()),
                context,
            )
            .await?;
        let call_result = hex::decode(call_result.trim_start_matches("0x"))
            .map_err(|e| Error::from(format!("failed to decode call result: {e}")))?;

        match bft_bridge_api::GET_WRAPPED_TOKEN
            .decode_output(&call_result)
            .map_err(|e| Error::from(format!("failed to decode call result: {e}")))?
            .as_slice()
        {
            &[Token::Address(token_address)] => Ok(token_address.into()),
            _ => Err(Error::InvalidTokenAddress),
        }
    }

    async fn query_burn_info(
        &self,
        user: &H160,
        operation_id: u32,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<BurnInfo> {
        let bft_bridge = context
            .borrow()
            .get_state()
            .config
            .get_bft_bridge_contract()
            .ok_or(Error::BftBridgeDoesNotExist)?;

        let signer = context.borrow().get_state().signer.get_transaction_signer();
        let minter_canister_address = signer
            .get_address()
            .await
            .map_err(|e| Error::from(format!("failed to get minter canister address: {e}")))?;

        let evmc = context.borrow().get_evm_canister();

        let get_burn_info_input = GET_PENDING_BURN_INFO
            .encode_input(&[Token::Address(user.0), Token::Uint(operation_id.into())])
            .map_err(|e| {
                Error::from(format!("failed to encode GET_PENDING_BURN_INFO input: {e}"))
            })?;

        let burn_info = evmc
            .eth_call(
                Some(minter_canister_address),
                Some(bft_bridge),
                None,
                DEFAULT_TX_GAS_LIMIT,
                None,
                Some(get_burn_info_input.into()),
                context,
            )
            .await?;

        let burn_info_str = burn_info.trim_start_matches("0x");
        let decoded_burn_info = hex::decode(burn_info_str).map_err(|e| {
            Error::InvalidBurnOperation(format!("failed to decode burn info from hex: {e}"))
        })?;

        let decoded_vec = GET_PENDING_BURN_INFO
            .decode_output(&decoded_burn_info)
            .map_err(|e| Error::InvalidBurnOperation(format!("failed to decode burn info: {e}")))?;

        let &[Token::Address(sender), Token::Uint(amount), Token::Address(from_erc_20), Token::FixedBytes(recipient), Token::FixedBytes(to_token), Token::FixedBytes(name), Token::FixedBytes(symbol), Token::Uint(decimals)] =
            &decoded_vec.as_slice()
        else {
            return Err(Error::InvalidBurnOperation("invalid burn info".into()));
        };

        let recipient_id = Id256::try_from(recipient.as_slice())?;
        let to_token = Id256::try_from(to_token.as_slice())?;

        let name = name
            .as_slice()
            .try_into()
            .map_err(|_| Error::InvalidBurnOperation("failed to decode name".into()))?;
        let symbol = symbol
            .as_slice()
            .try_into()
            .map_err(|_| Error::InvalidBurnOperation("failed to decode symbol".into()))?;
        if decimals > &ethers_core::types::U256::from(255u64) {
            return Err(Error::InvalidBurnOperation(format!(
                "decimals number too big: {decimals}"
            )));
        }

        Ok(BurnInfo {
            sender: (*sender).into(),
            amount: (*amount).into(),
            from_erc_20: (*from_erc_20).into(),
            recipient_id,
            to_token,
            name,
            symbol,
            decimals: decimals.as_u64() as _,
        })
    }
}

pub async fn sign_mint_order(
    order: MintOrder,
    context: &Rc<RefCell<dyn Context>>,
) -> Result<SignedMintOrder> {
    let signer = context.borrow().get_state().signer.get_transaction_signer();

    order.encode_and_sign(&signer).await
}

#[derive(Debug, Clone)]
struct BurnInfo {
    sender: H160,
    amount: U256,
    from_erc_20: H160,
    recipient_id: Id256,
    to_token: Id256,
    name: [u8; 32],
    symbol: [u8; 16],
    decimals: u8,
}

impl BurnInfo {
    /// Check if burn info has default values in fields.
    /// This means that burn info not found in BftBridge storage.
    pub fn is_dummy(&self) -> bool {
        self.amount.is_zero()
    }
}
