use std::fmt::Display;
use std::time::Duration;

use candid::{Nat, Principal};
use did::constant::EIP1559_INITIAL_BASE_FEE;
use did::state::BasicAccount;
use did::{TransactionReceipt, H160, H256, U256, U64};
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use eth_signer::{Signer, Wallet};
use ethers_core::abi::Token;
use ethers_core::k256::ecdsa::SigningKey;
use ethers_core::rand;
use evm_canister_client::{EvmCanisterClient, IcAgentClient};
use ic_exports::icrc_types::icrc1::account::Subaccount;
use ic_exports::icrc_types::icrc1::transfer::TransferArg;
use ic_exports::icrc_types::icrc2::approve::ApproveArgs;
use ic_test_utils::Agent;
use icrc_client::IcrcCanisterClient;
use minter_client::MinterCanisterClient;
use minter_contract_utils::bft_bridge_api::{BURN, DEPLOY_WRAPPED_TOKEN, GET_WRAPPED_TOKEN};
use minter_contract_utils::wrapped_token_api::{ERC_20_APPROVE, ERC_20_BALANCE};
use minter_did::id256::Id256;
use minter_did::reason::Icrc2Burn;
use tokio::time::Instant;

// use crate::icrc2_client::Icrc2CanisterClient;

pub const ICRC1_TRANSFER_FEE: u64 = 10_000;

/// Struct that encapsulates user operations with bridge and
/// simulates user actions.
#[derive(Clone)]
pub struct BridgeUser {
    name: String,
    principal: Principal,
    agent: Agent,
    evm_principal: Principal,
    minter_principal: Principal,
    wallet: Wallet<'static, SigningKey>,
    address: H160,
}

impl BridgeUser {
    pub fn new(
        name: String,
        agent: Agent,
        evm_principal: Principal,
        minter_principal: Principal,
    ) -> anyhow::Result<Self> {
        let principal = agent.get_principal().map_err(anyhow::Error::msg)?;
        let mut rng = rand::thread_rng();
        let wallet = Wallet::new(&mut rng);
        let address = wallet.address().into();
        Ok(Self {
            name,
            principal,
            agent,
            evm_principal,
            minter_principal,
            wallet,
            address,
        })
    }

    /// Get user's wallet.
    pub fn get_wallet(&self) -> &Wallet<'static, SigningKey> {
        &self.wallet
    }

    /// Start user's actions simulation for the specified duration.
    pub async fn run(self, icrc2_tokens: Vec<Principal>, test_duration: Duration) -> UserStats {
        log::trace!(
            "Start simulation for user {} for {} seconds",
            self.name,
            test_duration.as_secs()
        );

        let mut stats = UserStats::default();

        let erc20_address_futures = icrc2_tokens
            .iter()
            .map(|token| self.get_wrapped_token_address(*token));
        let address_results = futures::future::join_all(erc20_address_futures).await;

        let mut erc20_addresses = Vec::with_capacity(icrc2_tokens.len());
        for result in address_results {
            match result {
                Ok(address) => erc20_addresses.push(address),
                Err(e) => {
                    let msg = format!("failed to get wrapped token address: {e}");
                    stats.add_error(&msg, "getting wrapped token address");
                    return stats;
                }
            }
        }

        let tokens = Tokens {
            icrc2: icrc2_tokens,
            erc20: erc20_addresses,
        };
        let start = Instant::now();
        while Instant::now() - start < test_duration {
            let action = match self.select_next_action(&tokens).await {
                Ok(action) => action,
                Err(e) => {
                    log::warn!("No actions left for user {} due to {e}", self.name);
                    stats.add_error(&e, "selecting next action");
                    break;
                }
            };

            let action_stats = match self.perform_aciton(action, &tokens).await {
                Ok(action_stats) => action_stats,
                Err(e) => {
                    log::warn!("Action failed for user {}: {e}", self.name);
                    stats.add_error(&e, "performing user action");
                    continue;
                }
            };
            stats.merge(&action_stats);
        }

        stats
    }

    /// Mint BFT native tokens for user.
    pub async fn mint_bft_native_tokens(&self, amount: U256) -> anyhow::Result<U256> {
        log::trace!("minting {amount} BFT native tokens for user {}", self.name);

        Ok(self
            .evm_client()
            .mint_native_tokens(self.address.clone(), amount)
            .await??
            .1)
    }

    /// Get BFTBridge address.
    pub async fn get_bft_bridge_address(&self) -> anyhow::Result<H160> {
        let Some(address) = self.minter_client().get_bft_bridge_contract().await?? else {
            anyhow::bail!(
                "failed to get bft bridge address: bridge not registered in minter canister"
            );
        };
        Ok(address)
    }

    /// Get user's basic account.
    pub async fn get_account_basic(&self) -> anyhow::Result<BasicAccount> {
        Ok(self
            .evm_client()
            .account_basic(self.address.clone())
            .await?)
    }

    /// Get EVMc chain id.
    pub async fn get_chain_id(&self) -> anyhow::Result<u64> {
        Ok(self.evm_client().eth_chain_id().await?)
    }

    /// Create wrapped token for the given base ICRC-2 token by the user.
    pub async fn create_wrapped_token(
        &self,
        token: Principal,
        symbol: String,
    ) -> anyhow::Result<H160> {
        log::trace!("Creating wrapped token {} by user {}", symbol, self.name);

        let bft_bridge_address = self.get_bft_bridge_address().await?;

        let name = format!("Wrapped {symbol} token");
        let base_token_id = Id256::from(&token);
        let input = DEPLOY_WRAPPED_TOKEN.encode_input(&[
            Token::String(name),
            Token::String(symbol.clone()),
            Token::FixedBytes(base_token_id.0.to_vec()),
        ])?;

        let receipt = self
            .execute_transaction(bft_bridge_address, input, Self::CONTRACT_CALL_GAS)
            .await?;

        let Some(output) = receipt.output else {
            anyhow::bail!("wrapped token {} creation receipt has no output", symbol);
        };

        let decoded = DEPLOY_WRAPPED_TOKEN.decode_output(&output)?;
        let &[Token::Address(address)] = decoded.as_slice() else {
            anyhow::bail!("wrapped token {} creation output is incorrect", symbol);
        };

        Ok(address.into())
    }

    fn evm_client(&self) -> EvmCanisterClient<IcAgentClient> {
        let client = IcAgentClient::with_agent(self.evm_principal, self.agent.clone());
        EvmCanisterClient::new(client)
    }

    fn minter_client(&self) -> MinterCanisterClient<IcAgentClient> {
        let client = IcAgentClient::with_agent(self.minter_principal, self.agent.clone());
        MinterCanisterClient::new(client)
    }

    fn icrc2_client(&self, principal: Principal) -> IcrcCanisterClient<IcAgentClient> {
        let client = IcAgentClient::with_agent(principal, self.agent.clone());
        IcrcCanisterClient::new(client)
    }

    const CONTRACT_CALL_GAS: u64 = 10_000_000;

    async fn execute_transaction(
        &self,
        to: H160,
        input: Vec<u8>,
        gas: u64,
    ) -> anyhow::Result<TransactionReceipt> {
        let basic_account = self.get_account_basic().await?;
        let nonce = basic_account.nonce;
        let chain_id = self.get_chain_id().await?;

        let transaction = TransactionBuilder {
            from: &self.address,
            to: Some(to),
            nonce,
            value: U256::zero(),
            gas: gas.into(),
            gas_price: Some(EIP1559_INITIAL_BASE_FEE.into()),
            input,
            signature: SigningMethod::SigningKey(self.wallet.signer()),
            chain_id,
        }
        .calculate_hash_and_build()?;

        log::debug!(
            "Tx with hash {} sent with nonce {} by {}",
            transaction.hash,
            transaction.nonce,
            self.name
        );

        let hash = self
            .evm_client()
            .send_raw_transaction(transaction)
            .await??;

        let receipt = self.wait_for_tx_receipt(&hash).await?;

        if receipt.status != Some(U64::one()) {
            let output_data = receipt.output.as_deref().unwrap_or_default();
            let output = hex::encode(output_data);
            log::debug!(
                "Failed to execute transaction {}. Receipt: {:?}",
                hash,
                receipt
            );
            log::debug!("Output hex data: {}", output);
            anyhow::bail!("Failed to execute transaction {}.", hash,);
        }

        Ok(receipt)
    }

    async fn wait_for_tx_receipt(&self, hash: &H256) -> anyhow::Result<TransactionReceipt> {
        const RECEIPT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
        const RECEIPT_WAIT_DELAY: Duration = Duration::from_millis(200);

        let iterations = RECEIPT_WAIT_TIMEOUT.as_millis() / RECEIPT_WAIT_DELAY.as_millis();
        for i in 0..iterations {
            tokio::time::sleep(RECEIPT_WAIT_DELAY).await;

            log::trace!(
                "Waiting for transaction#{hash} receipt by {} user for {}ms.",
                self.name,
                i * RECEIPT_WAIT_DELAY.as_millis()
            );

            let receipt = self
                .evm_client()
                .eth_get_transaction_receipt(hash.clone())
                .await??;

            if let Some(receipt) = receipt {
                return Ok(receipt);
            }
        }

        anyhow::bail!(
            "failed to get transaction#{hash} receipt by {} user after waiting for {}ms.",
            self.name,
            RECEIPT_WAIT_TIMEOUT.as_millis()
        )
    }

    /// Get user's principal.
    pub fn get_principal(&self) -> Principal {
        self.principal
    }

    /// Transfer ICRC2 tokens to the specified user.
    pub async fn finish_icrc2_mint(
        &self,
        token: Principal,
        from_subaccount: Option<Subaccount>,
        to: &BridgeUser,
        amount: Nat,
    ) -> anyhow::Result<Nat> {
        log::trace!(
            "Transferring {amount} ICRC2 ({token}) tokens from {} to {}",
            self.name,
            to.name
        );

        let icrc2_client = self.icrc2_client(token);
        let transfer_args = TransferArg {
            from_subaccount,
            to: to.get_principal().into(),
            fee: None,
            created_at_time: None,
            memo: None,
            amount,
        };

        icrc2_client
            .icrc1_transfer(transfer_args)
            .await?
            .map_err(|e| anyhow::Error::msg(e.to_string()))
    }

    async fn select_next_action(&self, tokens: &Tokens) -> anyhow::Result<Action> {
        const MIN_BALANCE_TO_TRANSFER: u64 = 100_000;

        let idx = rand::random::<usize>() % tokens.len();
        let (_, erc20) = tokens.get_token_pair(idx).unwrap(); // idx < tokens.len()

        let erc20_balance = self.get_erc20_balance(erc20).await?;
        let transfer_amount = U256::from(MIN_BALANCE_TO_TRANSFER);
        let action = if erc20_balance > transfer_amount {
            Action::WithdrawErc20(idx, transfer_amount)
        } else {
            Action::DepositIcrc2(idx, (&transfer_amount).into())
        };
        Ok(action)
    }

    async fn get_wrapped_token_address(&self, base_token: Principal) -> anyhow::Result<H160> {
        log::trace!(
            "Getting wrapped token address for base token {base_token} by {}",
            self.name
        );
        let bft_bridge_address = self.get_bft_bridge_address().await?;

        let base_token_id = Id256::from(&base_token);
        let input =
            GET_WRAPPED_TOKEN.encode_input(&[Token::FixedBytes(base_token_id.0.to_vec())])?;

        let result = self
            .evm_client()
            .eth_call(
                Some(self.address.clone()),
                Some(bft_bridge_address),
                Some(0u64.into()),
                Self::CONTRACT_CALL_GAS,
                Some(EIP1559_INITIAL_BASE_FEE.into()),
                Some(input.into()),
            )
            .await??;

        let decoded_result = hex::decode(result.trim_start_matches("0x"))?;
        let decoded = &GET_WRAPPED_TOKEN.decode_output(&decoded_result)?;
        let &[Token::Address(token_address)] = decoded.as_slice() else {
            anyhow::bail!("failed to get wrapped token address");
        };

        Ok(token_address.into())
    }

    async fn get_erc20_balance(&self, erc20: H160) -> anyhow::Result<U256> {
        let input = ERC_20_BALANCE.encode_input(&[Token::Address(self.address.0)])?;

        let result = self
            .evm_client()
            .eth_call(
                Some(self.address.clone()),
                Some(erc20),
                Some(0u64.into()),
                Self::CONTRACT_CALL_GAS,
                Some(EIP1559_INITIAL_BASE_FEE.into()),
                Some(input.into()),
            )
            .await??;

        let decoded_result = hex::decode(result.trim_start_matches("0x"))?;
        let decoded = &ERC_20_BALANCE.decode_output(&decoded_result)?;
        let &[Token::Uint(balance)] = decoded.as_slice() else {
            anyhow::bail!("failed to get wrapped token address");
        };

        Ok(balance.into())
    }

    async fn perform_aciton(&self, action: Action, tokens: &Tokens) -> anyhow::Result<UserStats> {
        let mut stats = UserStats::default();
        match action {
            Action::DepositIcrc2(token_idx, amount) => {
                let icrc2_token = tokens.icrc2[token_idx];
                self.deposit_icrc2(icrc2_token, &amount).await?;
                stats.icrc2_total_deposit_amount += amount;
                stats.icrc2_deposits_number += 1;
            }
            Action::WithdrawErc20(token_idx, amount) => {
                let (icrc2_token, erc20_token) = tokens.get_token_pair(token_idx).unwrap(); // index always < tokens.len()
                self.withdraw_erc20(icrc2_token, erc20_token, &amount)
                    .await?;
                stats.erc20_total_withdraw_amount += Nat::from(&amount);
                stats.erc20_withdrawals_number += 1;
            }
        };

        Ok(stats)
    }

    async fn deposit_icrc2(&self, icrc2_token: Principal, amount: &Nat) -> anyhow::Result<()> {
        log::trace!(
            "Depositing {amount} ICRC2 ({icrc2_token}) tokens by {} user",
            self.name
        );

        let amount_with_fee = amount.clone() + Nat::from(ICRC1_TRANSFER_FEE);
        self.approve_icrc2_burn(icrc2_token, amount_with_fee)
            .await?;

        let reason = Icrc2Burn {
            amount: amount.try_into().unwrap(), // amount always less then U256::MAX
            from_subaccount: None,
            icrc2_token_principal: icrc2_token,
            recipient_address: self.address.clone(),
            operation_id: 0,
            approve_minted_tokens: None,
        };

        log::trace!("Burning ICRC-2 tokens by {} user", self.name);
        let _operation_id = self.minter_client().burn_icrc2(reason).await??;

        Ok(())
    }

    async fn withdraw_erc20(
        &self,
        icrc2_token: Principal,
        erc20_token: H160,
        amount: &U256,
    ) -> anyhow::Result<()> {
        log::trace!(
            "Withdrawing {amount} ERC-20 ({icrc2_token}) tokens by {} user",
            self.name
        );

        let bft_bridge_address = self.get_bft_bridge_address().await?;

        self.approve_erc20_burn(erc20_token.clone(), amount, &bft_bridge_address)
            .await?;
        let _operation_id = self
            .burn_erc20(&erc20_token, amount, bft_bridge_address.clone())
            .await?;

        let amount_without_fee = Nat::from(amount) - Nat::from(ICRC1_TRANSFER_FEE * 2);
        log::trace!(
            "Transferring {amount_without_fee} ICRC-2 {icrc2_token} tokens by {} user",
            self.name
        );

        Ok(())
    }

    async fn approve_icrc2_burn(&self, icrc2_token: Principal, amount: Nat) -> anyhow::Result<()> {
        log::trace!(
            "Approving ICRC-2 token burn for {amount} by {} user",
            self.name
        );

        let approve_args = ApproveArgs {
            from_subaccount: None,
            spender: self.minter_principal.into(),
            amount,
            expected_allowance: None,
            expires_at: None,
            fee: None,
            memo: None,
            created_at_time: None,
        };
        self.icrc2_client(icrc2_token)
            .icrc2_approve(approve_args)
            .await?
            .map_err(|e| anyhow::Error::msg(format!("failed to approve ICRC-2 token: {e:?}")))?;
        Ok(())
    }

    async fn approve_erc20_burn(
        &self,
        erc20_token: H160,
        amount: &U256,
        bft_bridge_address: &H160,
    ) -> anyhow::Result<()> {
        log::trace!(
            "Approving ERC-20 token burn for {amount} by {} user",
            self.name
        );

        let input = ERC_20_APPROVE
            .encode_input(&[Token::Address(bft_bridge_address.0), Token::Uint(amount.0)])?;

        let receipt = self
            .execute_transaction(erc20_token, input, Self::CONTRACT_CALL_GAS)
            .await?;
        let Some(output) = receipt.output else {
            anyhow::bail!("burn_erc20 transaction output is empty");
        };

        let decoded = &ERC_20_APPROVE.decode_output(&output)?;
        let &[Token::Bool(success)] = decoded.as_slice() else {
            anyhow::bail!("failed to get wrapped token address");
        };

        if !success {
            anyhow::bail!("failed to approve ERC-20 token transfer by {}", self.name);
        }

        Ok(())
    }

    async fn burn_erc20(
        &self,
        erc20_token: &H160,
        amount: &U256,
        bft_bridge_address: H160,
    ) -> anyhow::Result<u32> {
        log::trace!("Burning ERC-20 tokens by {} user", self.name);

        let recipient = Id256::from(&self.principal);
        let input = BURN.encode_input(&[
            Token::Uint(amount.0),
            Token::Address(erc20_token.0),
            Token::Bytes(recipient.0.to_vec()),
        ])?;

        let receipt = self
            .execute_transaction(bft_bridge_address, input, Self::CONTRACT_CALL_GAS)
            .await?;
        let Some(output) = receipt.output else {
            anyhow::bail!("burn_erc20 transaction output is empty");
        };

        let decoded = &BURN.decode_output(&output)?;
        let &[Token::Uint(operation_id)] = decoded.as_slice() else {
            anyhow::bail!(
                "failed to decode output of burn ERC-20 transaction by {}",
                self.name
            );
        };

        Ok(operation_id.as_u32())
    }
}

/// Represents statistics of bridge operations.
#[derive(Debug, Default)]
pub struct UserStats {
    pub icrc2_deposits_number: u32,
    pub icrc2_total_deposit_amount: Nat,
    pub erc20_withdrawals_number: u32,
    pub erc20_total_withdraw_amount: Nat,
    pub unexpected_errors: Vec<String>,
}

impl UserStats {
    /// Add anonther statistics to `self`.
    pub fn merge(&mut self, other: &Self) {
        self.icrc2_deposits_number += other.icrc2_deposits_number;
        self.icrc2_total_deposit_amount += other.icrc2_total_deposit_amount.clone();
        self.erc20_withdrawals_number += other.erc20_withdrawals_number;
        self.erc20_total_withdraw_amount += other.erc20_total_withdraw_amount.clone();
        self.unexpected_errors
            .extend_from_slice(&other.unexpected_errors);
    }

    /// Add an error to statistics.
    pub fn add_error(&mut self, error: &impl Display, operation: &str) {
        let msg = format!("unexpected error during {} operation: {}", operation, error);
        self.unexpected_errors.push(msg)
    }
}

type TokenIdx = usize;

enum Action {
    DepositIcrc2(TokenIdx, Nat),
    WithdrawErc20(TokenIdx, U256),
}

struct Tokens {
    pub icrc2: Vec<Principal>,
    pub erc20: Vec<H160>,
}

impl Tokens {
    pub fn get_token_pair(&self, idx: usize) -> Option<(Principal, H160)> {
        self.icrc2
            .get(idx)
            .and_then(|icrc2| Some((*icrc2, self.erc20.get(idx)?.clone())))
    }

    fn len(&self) -> usize {
        self.icrc2.len()
    }
}
