pub mod brc20_bridge;
pub mod btc_bridge;
mod evm_rpc_canister;
pub mod rune_bridge;
pub mod stress;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bridge_did::error::BTFResult as McResult;
use bridge_did::id256::Id256;
use bridge_did::init::brc20::{Brc20BridgeConfig, SchnorrKeyIds};
use bridge_did::init::btc::BitcoinConnection;
use bridge_did::init::erc20::{BaseEvmSettings, QueryDelays};
use bridge_did::operation_log::Memo;
use bridge_did::order::SignedOrders;
use bridge_did::reason::{ApproveAfterMint, Icrc2Burn};
use bridge_utils::evm_link::address_to_icrc_subaccount;
use bridge_utils::{BTFBridge, FeeCharge, UUPSProxy, WrappedToken, WrappedTokenDeployer};
use candid::utils::ArgumentEncoder;
use candid::{Encode, Nat, Principal};
use did::constant::EIP1559_INITIAL_BASE_FEE;
use did::error::EvmError;
use did::init::EvmCanisterInitData;
use did::{Transaction, TransactionReceipt, H160, H256, U256, U64};
use eth_signer::ic_sign::SigningKeyId;
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use eth_signer::LocalWallet;
use evm_canister_client::CanisterClient;
use evm_rpc_canister::EvmRpcCanisterInitData;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_exports::icrc_types::icrc::generic_metadata_value::MetadataValue;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_exports::icrc_types::icrc1_ledger::{
    ArchiveOptions, FeatureFlags, InitArgs, LedgerArgument,
};
use ic_exports::icrc_types::icrc2::approve::ApproveArgs;
use icrc2_bridge::SigningStrategy;
use icrc_client::IcrcCanisterClient;
use tokio::time::Instant;

use super::utils::error::Result;
use crate::utils::btc::{BtcNetwork, InitArg, KytMode, LifecycleArg, MinterArg, Mode};
use crate::utils::error::TestError;
use crate::utils::wasm::*;
use crate::utils::{TestEvm, TestWTM, CHAIN_ID, EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS};

pub const DEFAULT_GAS_PRICE: u128 = EIP1559_INITIAL_BASE_FEE * 2;
const BITCOIN_CANISTER_ID: &str = "g4xu7-jiaaa-aaaan-aaaaq-cai";
const NNS_ROOT_CANISTER_ID: &str = "r7inp-6aaaa-aaaaa-aaabq-cai";

use alloy_sol_types::{SolCall, SolConstructor};
use bridge_client::{
    Brc20BridgeClient, BtcBridgeClient, Erc20BridgeClient, Icrc2BridgeClient, RuneBridgeClient,
};
use bridge_did::event_data::MinterNotificationType;
use bridge_did::evm_link::EvmLink;
use bridge_did::init::{BridgeInitData, BtcBridgeConfig, IndexerType, RuneBridgeConfig};
use bridge_did::op_id::OperationId;
use ic_log::did::LogCanisterSettings;

#[async_trait::async_trait]
pub trait TestContext<EVM>
where
    EVM: TestEvm,
{
    type Client: CanisterClient + Send + Sync;

    /// Returns principals for canisters in the context.
    fn canisters(&self) -> TestCanisters;

    /// Returns client for the canister.
    fn client(&self, canister: Principal, caller: &str) -> Self::Client;

    /// Returns principal by caller's name.
    fn principal_by_caller_name(&self, caller: &str) -> Principal;

    /// Principal to use for canisters initialization.
    fn admin(&self) -> Principal;

    /// Principal to use for canisters initialization.
    fn admin_name(&self) -> &str;

    /// Signing key to use for management canister signing.
    fn sign_key(&self) -> SigningKeyId;

    /// Returns the base EVM LINK
    fn base_evm_link(&self) -> EvmLink;

    /// Returns the external EVM LINK
    fn wrapped_evm_link(&self) -> EvmLink;

    /// Returns a reference to the base EVM instance.
    fn base_evm(&self) -> Arc<EVM>;

    /// Returns a reference to the wrapped EVM instance.
    fn wrapped_evm(&self) -> Arc<EVM>;

    /// Returns client for the icrc2 bridge
    fn icrc_bridge_client(&self, caller: &str) -> Icrc2BridgeClient<Self::Client> {
        Icrc2BridgeClient::new(self.client(self.canisters().icrc2_bridge(), caller))
    }

    /// Returns client for the erc20 bridge
    fn erc20_bridge_client(&self, caller: &str) -> Erc20BridgeClient<Self::Client> {
        Erc20BridgeClient::new(self.client(self.canisters().erc20_bridge(), caller))
    }

    fn rune_bridge_client(&self, caller: &str) -> RuneBridgeClient<Self::Client> {
        RuneBridgeClient::new(self.client(self.canisters().rune_bridge(), caller))
    }

    fn btc_bridge_client(&self, caller: &str) -> BtcBridgeClient<Self::Client> {
        BtcBridgeClient::new(self.client(self.canisters().btc_bridge(), caller))
    }

    fn brc20_bridge_client(&self, caller: &str) -> Brc20BridgeClient<Self::Client> {
        Brc20BridgeClient::new(self.client(self.canisters().brc20_bridge(), caller))
    }

    /// Returns client for the ICRC token canister.
    fn icrc_token_client(
        &self,
        canister: Principal,
        caller: &str,
    ) -> IcrcCanisterClient<Self::Client> {
        IcrcCanisterClient::new(self.client(canister, caller))
    }

    /// Returns client for the ICRC ckbtc ledger canister.
    fn ckbtc_token_client(&self, caller: &str) -> IcrcCanisterClient<Self::Client> {
        self.icrc_token_client(self.canisters().ckbtc_ledger(), caller)
    }

    /// Returns client for the ICRC token 1 canister.
    fn icrc_token_1_client(&self, caller: &str) -> IcrcCanisterClient<Self::Client> {
        self.icrc_token_client(self.canisters().token_1(), caller)
    }

    /// Returns client for the ICRC token 2 canister.
    fn icrc_token_2_client(&self, caller: &str) -> IcrcCanisterClient<Self::Client> {
        self.icrc_token_client(self.canisters().token_2(), caller)
    }

    /// Waits for transaction receipt.
    async fn wait_transaction_receipt(&self, hash: &H256) -> Result<Option<TransactionReceipt>> {
        self.wait_transaction_receipt_on_evm(&self.wrapped_evm(), hash)
            .await
    }

    /// Waits for transaction receipt.
    async fn wait_transaction_receipt_on_evm(
        &self,
        evm: &Arc<EVM>,
        hash: &H256,
    ) -> Result<Option<TransactionReceipt>> {
        let tx_processing_interval = EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS;
        let timeout = tx_processing_interval * 100;
        let start = Instant::now();

        let mut receipt = None;

        while start.elapsed() < timeout && receipt.is_none() {
            self.advance_time(tx_processing_interval).await;
            receipt = evm.get_transaction_receipt(hash).await?;
        }
        Ok(receipt)
    }

    async fn advance_time(&self, time: Duration);

    /// Advances time by `duration` `times` times.
    async fn advance_by_times(&self, duration: Duration, times: u64) {
        for _ in 0..=times {
            self.advance_time(duration).await;
        }
    }

    /// Creates a new wallet with the EVM balance on it.
    async fn new_wallet(&self, balance: u128) -> Result<LocalWallet> {
        self.new_wallet_on_evm(&self.wrapped_evm(), balance).await
    }

    /// Creates a new wallet with the EVM balance on it.
    async fn new_wallet_on_evm(&self, evm: &Arc<EVM>, balance: u128) -> Result<LocalWallet> {
        let wallet = LocalWallet::random();
        evm.mint_native_tokens(wallet.address().into(), balance.into())
            .await?;

        self.advance_time(Duration::from_secs(2)).await;

        Ok(wallet)
    }

    /// Returns bridge canister EVM address.
    async fn get_icrc_bridge_canister_evm_address(&self, caller: &str) -> Result<H160> {
        let client = self.client(self.canisters().icrc2_bridge(), caller);
        Ok(client
            .update::<_, McResult<H160>>("get_bridge_canister_evm_address", ())
            .await??)
    }

    /// Creates contract in EVMc.
    async fn create_contract(&self, creator_wallet: &LocalWallet, input: Vec<u8>) -> Result<H160> {
        self.create_contract_on_evm(&self.wrapped_evm(), creator_wallet, input)
            .await
    }

    /// Creates contract on the given EVM.
    async fn create_contract_on_evm(
        &self,
        evm: &Arc<EVM>,
        creator_wallet: &LocalWallet,
        input: Vec<u8>,
    ) -> Result<H160> {
        let creator_address: H160 = creator_wallet.address().into();
        let nonce = evm.get_next_nonce(&creator_address).await?;

        let create_contract_tx = self
            .signed_transaction(evm, creator_wallet, None, nonce, 0, input)
            .await;
        println!("tx {:?}", create_contract_tx);

        let hash = evm.send_raw_transaction(create_contract_tx).await?;
        println!("Contract creation tx hash: {hash}",);
        let receipt = self
            .wait_transaction_receipt_on_evm(evm, &hash)
            .await?
            .ok_or(TestError::Evm(EvmError::Internal(
                "transaction not processed".into(),
            )))?;

        if receipt.status != Some(U64::from(1u64)) {
            println!("tx status: {:?}", receipt.status);
            dbg!(&receipt);
            dbg!(&hex::encode(receipt.output.as_ref().unwrap_or(&vec![])));
            Err(TestError::Evm(EvmError::Internal(
                "contract creation failed".into(),
            )))
        } else {
            Ok(receipt.contract_address.expect(
                "contract creation transaction succeeded, but it doesn't contain the contract address",
            ))
        }
    }

    /// Crates BTFBridge contract in EVMc and registered it in minter canister
    async fn initialize_btf_bridge(
        &self,
        minter_canister_address: H160,
        fee_charge_address: Option<H160>,
        wrapped_token_deployer: H160,
    ) -> Result<H160> {
        self.initialize_btf_bridge_on_evm(
            &self.wrapped_evm(),
            minter_canister_address,
            fee_charge_address,
            wrapped_token_deployer,
            true,
        )
        .await
    }

    /// Crates BTFBridge contract in EVMc and registered it in minter canister
    async fn initialize_btf_bridge_on_evm(
        &self,
        evm: &Arc<EVM>,
        minter_canister_address: H160,
        fee_charge_address: Option<H160>,
        wrapped_token_deployer: H160,
        is_wrapped: bool,
    ) -> Result<H160> {
        evm.mint_native_tokens(minter_canister_address.clone(), u64::MAX.into())
            .await?;
        self.advance_time(Duration::from_secs(2)).await;

        let wallet = self.new_wallet_on_evm(evm, u64::MAX.into()).await?;
        let bridge_address = self
            .initialize_btf_bridge_with_minter_on_evm(
                evm,
                &wallet,
                minter_canister_address,
                fee_charge_address,
                wrapped_token_deployer,
                is_wrapped,
            )
            .await?;

        Ok(bridge_address)
    }

    /// Creates BTFBridge contract in EVMC and registered it in minter canister
    async fn initialize_btf_bridge_with_minter(
        &self,
        wallet: &LocalWallet,
        minter_canister_address: H160,
        fee_charge_address: Option<H160>,
        wrapped_token_deployer: H160,
        is_wrapped: bool,
    ) -> Result<H160> {
        self.initialize_btf_bridge_with_minter_on_evm(
            &self.wrapped_evm(),
            wallet,
            minter_canister_address,
            fee_charge_address,
            wrapped_token_deployer,
            is_wrapped,
        )
        .await
    }

    /// Creates BTFBridge contract in EVMC and registered it in minter canister
    async fn initialize_btf_bridge_with_minter_on_evm(
        &self,
        evm: &Arc<EVM>,
        wallet: &LocalWallet,
        minter_canister_address: H160,
        fee_charge_address: Option<H160>,
        wrapped_token_deployer: H160,
        is_wrapped: bool,
    ) -> Result<H160> {
        let mut bridge_input = BTFBridge::BYTECODE.to_vec();
        let constructor = BTFBridge::constructorCall {}.abi_encode();
        bridge_input.extend_from_slice(&constructor);

        println!("bridge bytecode size: {}", BTFBridge::BYTECODE.len());

        let bridge_address = self
            .create_contract_on_evm(evm, wallet, bridge_input.clone())
            .await
            .unwrap();

        let init_data = BTFBridge::initializeCall {
            minterAddress: minter_canister_address.into(),
            feeChargeAddress: fee_charge_address.unwrap_or_default().into(),
            wrappedTokenDeployer: wrapped_token_deployer.into(),
            isWrappedSide: is_wrapped,
            owner: [0; 20].into(),
            controllers: vec![],
        }
        .abi_encode();

        let mut proxy_input = UUPSProxy::BYTECODE.to_vec();
        let constructor = UUPSProxy::constructorCall {
            _implementation: bridge_address.into(),
            _data: init_data.into(),
        }
        .abi_encode();
        proxy_input.extend_from_slice(&constructor);

        let proxy_address = self
            .create_contract_on_evm(evm, wallet, proxy_input)
            .await
            .unwrap();

        println!("proxy_address: {}", proxy_address);

        Ok(proxy_address)
    }

    async fn initialize_fee_charge_contract(
        &self,
        wallet: &LocalWallet,
        minter_canister_addresses: &[H160],
    ) -> Result<H160> {
        self.initialize_fee_charge_contract_on_evm(
            &self.wrapped_evm(),
            wallet,
            minter_canister_addresses,
        )
        .await
    }

    async fn initialize_fee_charge_contract_on_evm(
        &self,
        evm: &Arc<EVM>,
        wallet: &LocalWallet,
        minter_canister_addresses: &[H160],
    ) -> Result<H160> {
        let minter_canister_addresses = minter_canister_addresses
            .iter()
            .map(|addr| addr.clone().into())
            .collect();

        let mut fee_charge_input = FeeCharge::BYTECODE.to_vec();

        let input = FeeCharge::constructorCall {
            canChargeFee: minter_canister_addresses,
        }
        .abi_encode();

        fee_charge_input.extend_from_slice(&input);

        let fee_charge_address = self
            .create_contract_on_evm(evm, wallet, fee_charge_input.clone())
            .await
            .unwrap();

        Ok(fee_charge_address)
    }

    async fn initialize_wrapped_token_deployer_contract(
        &self,
        wallet: &LocalWallet,
    ) -> Result<H160> {
        self.initialize_wrapped_token_deployer_contract_on_evm(&self.wrapped_evm(), wallet)
            .await
    }

    async fn initialize_wrapped_token_deployer_contract_on_evm(
        &self,
        evm: &Arc<EVM>,
        wallet: &LocalWallet,
    ) -> Result<H160> {
        let wrapped_token_deployer_input = WrappedTokenDeployer::BYTECODE.to_vec();

        self.create_contract_on_evm(evm, wallet, wrapped_token_deployer_input.clone())
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn burn_erc_20_tokens_raw(
        &self,
        evm: &Arc<EVM>,
        wallet: &LocalWallet,
        from_token: &H160,
        to_token_id: &[u8],
        recipient: Vec<u8>,
        bridge: &H160,
        amount: u128,
        wrapped: bool,
        memo: Option<Memo>,
    ) -> Result<(u32, H256)> {
        let amount: U256 = amount.into();

        if !wrapped {
            let input = WrappedToken::approveCall {
                spender: bridge.clone().into(),
                value: amount.clone().into(),
            }
            .abi_encode();

            println!("abi encoded; calling evm");

            let results = self
                .call_contract_on_evm(evm, wallet, &from_token.clone(), input, 0)
                .await?;
            let output = results.1.output.unwrap();
            let decoded_output =
                WrappedToken::approveCall::abi_decode_returns(&output, true).unwrap();

            assert!(decoded_output._0);
        }

        println!("Burning src tokens using Btfbridge");

        let input = BTFBridge::burnCall {
            amount: amount.into(),
            fromERC20: from_token.clone().into(),
            toTokenID: alloy_sol_types::private::FixedBytes::from_slice(to_token_id),
            recipientID: recipient.into(),
            memo: memo.map(|m| m.into()).unwrap_or_default(),
        }
        .abi_encode();

        let (tx_hash, receipt) = self
            .call_contract_on_evm(evm, wallet, bridge, input, 0)
            .await?;

        if receipt.status != Some(U64::from(1u64)) {
            let decoded_output =
                BTFBridge::burnCall::abi_decode_returns(&receipt.output.clone().unwrap(), false)
                    .unwrap();
            return Err(TestError::Generic(format!(
                "Burn transaction failed: {decoded_output:?} -- {receipt:?}, -- {}",
                String::from_utf8_lossy(receipt.output.as_ref().unwrap())
            )));
        }

        let decoded_output =
            BTFBridge::burnCall::abi_decode_returns(&dbg!(receipt.output.clone()).unwrap(), true)
                .unwrap();

        let operation_id = decoded_output._0;
        Ok((operation_id, tx_hash))
    }

    #[allow(clippy::too_many_arguments)]
    async fn burn_wrapped_erc_20_tokens(
        &self,
        evm: &Arc<EVM>,
        wallet: &LocalWallet,
        from_token: &H160,
        to_token_id: &[u8],
        recipient: Id256,
        bridge: &H160,
        amount: u128,
    ) -> Result<(u32, H256)> {
        self.burn_erc_20_tokens_raw(
            evm,
            wallet,
            from_token,
            to_token_id,
            recipient.0.to_vec(),
            bridge,
            amount,
            true,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn burn_base_erc_20_tokens(
        &self,
        evm: &Arc<EVM>,
        wallet: &LocalWallet,
        from_token: &H160,
        to_token_id: &[u8],
        recipient: Id256,
        bridge: &H160,
        amount: u128,
        memo: Option<Memo>,
    ) -> Result<(u32, H256)> {
        self.burn_erc_20_tokens_raw(
            evm,
            wallet,
            from_token,
            to_token_id,
            recipient.0.to_vec(),
            bridge,
            amount,
            false,
            memo,
        )
        .await
    }

    /// Current native token balance on user's deposit inside the Btfbridge.
    async fn native_token_deposit_balance(
        &self,
        evm: &Arc<EVM>,
        fee_charge: H160,
        user: H160,
    ) -> U256 {
        let input = FeeCharge::nativeTokenBalanceCall {
            user: user.clone().into(),
        }
        .abi_encode();
        let response = evm
            .eth_call(
                Some(user),
                Some(fee_charge),
                None,
                3_000_000,
                None,
                Some(input.into()),
            )
            .await
            .expect("Failed to get native token balance");

        let balance = FeeCharge::nativeTokenBalanceCall::abi_decode_returns(&response, true)
            .unwrap()
            .balance
            .into();

        balance
    }

    /// Deposit native tokens to Btfbridge to pay mint fee.
    async fn native_token_deposit(
        &self,
        evm: &Arc<EVM>,
        fee_charge: H160,
        user_wallet: &LocalWallet,
        amount: u128,
    ) -> Result<U256> {
        let input = FeeCharge::nativeTokenDepositCall {}.abi_encode();

        let receipt = self
            .call_contract_on_evm(evm, user_wallet, &fee_charge, input, amount)
            .await?
            .1;

        let new_balance = FeeCharge::nativeTokenDepositCall::abi_decode_returns(
            receipt.output.as_ref().unwrap(),
            true,
        )
        .unwrap()
        .balance;

        Ok(new_balance.into())
    }

    /// Returns a signed transaction from the given `wallet`.
    async fn signed_transaction(
        &self,
        evm: &Arc<EVM>,
        wallet: &LocalWallet,
        to: Option<H160>,
        nonce: U256,
        value: u128,
        input: Vec<u8>,
    ) -> Transaction {
        let address = wallet.address();
        TransactionBuilder {
            from: &address.into(),
            to,
            nonce,
            value: value.into(),
            gas: 8_000_000u64.into(),
            gas_price: DEFAULT_GAS_PRICE.into(),
            input,
            signature: SigningMethod::SigningKey(wallet.credential()),
            chain_id: evm.chain_id().await.expect("Failed to get chain id"),
        }
        .calculate_hash_and_build()
        .unwrap()
    }

    /// Calls contract in EVMc.
    async fn call_contract(
        &self,
        wallet: &LocalWallet,
        contract: &H160,
        input: Vec<u8>,
        amount: u128,
    ) -> Result<(H256, TransactionReceipt)> {
        self.call_contract_on_evm(&self.wrapped_evm(), wallet, contract, input, amount)
            .await
    }

    /// Calls contract in the evm_client.
    async fn call_contract_on_evm(
        &self,
        evm: &Arc<EVM>,
        wallet: &LocalWallet,
        contract: &H160,
        input: Vec<u8>,
        amount: u128,
    ) -> Result<(H256, TransactionReceipt)> {
        let from: H160 = wallet.address().into();
        let nonce = evm.get_next_nonce(&from).await?;

        let output = evm
            .eth_call(
                Some(from),
                Some(contract.clone()),
                Some(amount.into()),
                8_000_000u64.into(),
                Some(DEFAULT_GAS_PRICE.into()),
                Some(input.clone().into()),
            )
            .await?;

        let call_tx = self
            .signed_transaction(evm, wallet, Some(contract.clone()), nonce, amount, input)
            .await;

        let hash = evm.send_raw_transaction(call_tx).await?;
        let mut receipt = self
            .wait_transaction_receipt_on_evm(evm, &hash)
            .await?
            .ok_or(TestError::Evm(EvmError::Internal(
                "transaction not processed".into(),
            )))?;

        if receipt.status != Some(U64::from(1u64)) {
            println!("tx status: {:?}", receipt.status);
            dbg!(&receipt);
            dbg!(&hex::encode(receipt.output.as_ref().unwrap_or(&vec![])));
        }

        receipt.output = receipt.output.or(Some(output));

        Ok((hash, receipt))
    }

    /// Calls contract in the evm_client without waiting for it's receipt.
    async fn call_contract_without_waiting(
        &self,
        wallet: &LocalWallet,
        contract: &H160,
        input: Vec<u8>,
        amount: u128,
        nonce: Option<u64>,
    ) -> Result<H256> {
        self.call_contract_without_waiting_on_evm(
            &self.wrapped_evm(),
            wallet,
            contract,
            input,
            amount,
            nonce,
        )
        .await
    }

    /// Calls contract in the evm_client without waiting for it's receipt.
    async fn call_contract_without_waiting_on_evm(
        &self,
        evm: &Arc<EVM>,
        wallet: &LocalWallet,
        contract: &H160,
        input: Vec<u8>,
        amount: u128,
        nonce: Option<u64>,
    ) -> Result<H256> {
        let from: H160 = wallet.address().into();
        let nonce = match nonce {
            Some(n) => n.into(),
            None => evm.get_next_nonce(&from).await?,
        };

        println!("sending tx from wallet {from} with nonce {nonce}");
        let call_tx = self
            .signed_transaction(evm, wallet, Some(contract.clone()), nonce, amount, input)
            .await;

        let hash = evm.send_raw_transaction(call_tx).await?;

        Ok(hash)
    }

    /// Creates wrapped token in EVMc by calling `BTFBridge:::deploy_wrapped_token()`.
    async fn create_wrapped_token(
        &self,
        wallet: &LocalWallet,
        btf_bridge: &H160,
        base_token_id: Id256,
    ) -> Result<H160> {
        let input = BTFBridge::deployERC20Call {
            name: "Wrapper".into(),
            symbol: "WPT".into(),
            decimals: 18,
            baseTokenID: base_token_id.0.into(),
        }
        .abi_encode();

        let (_hash, receipt) = self.call_contract(wallet, btf_bridge, input, 0).await?;

        let output = receipt.output.as_ref().ok_or_else(|| {
            println!("receipt: {:?}", receipt);
            TestError::Generic("No output in receipt".into())
        })?;

        let address = BTFBridge::deployERC20Call::abi_decode_returns(output, true)
            .unwrap()
            ._0;

        println!(
            "Deployed Wrapped token on block {} with address {address}",
            receipt.block_number
        );
        Ok(address.into())
    }

    /// Deploys TestWTM token of the given EVM.
    async fn deploy_test_wtm_token_on_evm(
        &self,
        evm: &Arc<EVM>,
        wallet: &LocalWallet,
        init_balance: U256,
    ) -> Result<H160> {
        let mut erc20_input = TestWTM::BYTECODE.to_vec();
        let constructor = TestWTM::constructorCall {
            initialSupply: init_balance.into(),
        }
        .abi_encode();
        erc20_input.extend_from_slice(&constructor);

        let deployer_address = wallet.address();
        let nonce = evm.get_next_nonce(&deployer_address.into()).await?;
        let tx = self
            .signed_transaction(evm, wallet, None, nonce, 0, erc20_input)
            .await;
        let hash = evm
            .send_raw_transaction(tx)
            .await
            .expect("Failed to deploy TestWTM token");

        let receipt = self
            .wait_transaction_receipt_on_evm(evm, &hash)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(receipt.status, Some(U64::from(1u64)));

        Ok(receipt.contract_address.unwrap())
    }

    /// Burns ICRC-2 token 1 and creates according mint order.
    #[allow(clippy::too_many_arguments)]
    async fn burn_icrc2(
        &self,
        caller: &str,
        wallet: &LocalWallet,
        bridge: &H160,
        erc20_token_address: &H160,
        amount: u128,
        fee_payer: Option<H160>,
        approve_after_mint: Option<ApproveAfterMint>,
    ) -> Result<()> {
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

        let input = BTFBridge::notifyMinterCall {
            notificationType: MinterNotificationType::DepositRequest as u32,
            userData: encoded_reason.into(),
            memo: alloy_sol_types::private::FixedBytes::ZERO,
        }
        .abi_encode();

        let _receipt = self
            .call_contract(wallet, bridge, input, 0)
            .await
            .map(|(_, receipt)| receipt)?;

        Ok(())
    }

    async fn reschedule_operation(
        &self,
        operation_id: OperationId,
        wallet: &LocalWallet,
        bridge: &H160,
    ) -> Result<()> {
        let encoded_op_id = Encode!(&operation_id).unwrap();
        let input = BTFBridge::notifyMinterCall {
            notificationType: MinterNotificationType::RescheduleOperation as u32,
            userData: encoded_op_id.into(),
            memo: alloy_sol_types::private::FixedBytes::ZERO,
        }
        .abi_encode();

        let _receipt = self
            .call_contract(wallet, bridge, input, 0)
            .await
            .map(|(_, receipt)| receipt)?;

        Ok(())
    }

    /// Approves burning of ICRC-2 token.
    async fn approve_icrc2_burn(&self, caller: &str, recipient: &H160, amount: u128) -> Result<()> {
        let client = self.icrc_token_1_client(caller);

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
    }

    /// Mints ERC-20 token with the order.
    async fn batch_mint_erc_20_with_order(
        &self,
        wallet: &LocalWallet,
        bridge: &H160,
        order: SignedOrders,
    ) -> Result<TransactionReceipt> {
        let all_orders = order.all_orders().clone();
        let input = BTFBridge::batchMintCall {
            encodedOrders: all_orders.orders_data.into(),
            signature: all_orders.signature.into(),
            ordersToProcess: vec![order.idx() as u32],
        }
        .abi_encode();

        self.call_contract(wallet, bridge, input, 0)
            .await
            .map(|(_, receipt)| receipt)
    }

    /// Returns ERC-20 balance.
    async fn check_erc20_balance(
        &self,
        token: &H160,
        wallet: &LocalWallet,
        address: Option<&H160>,
    ) -> Result<u128> {
        self.check_erc20_balance_on_evm(&self.wrapped_evm(), token, wallet, address)
            .await
    }

    /// Returns ERC-20 balance on the given evm.
    async fn check_erc20_balance_on_evm(
        &self,
        evm: &Arc<EVM>,
        token: &H160,
        wallet: &LocalWallet,
        address: Option<&H160>,
    ) -> Result<u128> {
        let account = address.cloned().unwrap_or_else(|| wallet.address().into());
        let input = WrappedToken::balanceOfCall {
            account: account.into(),
        }
        .abi_encode();

        let response = evm
            .eth_call(
                Some(wallet.address().into()),
                Some(token.clone()),
                None,
                3_000_000,
                None,
                Some(input.into()),
            )
            .await?;

        let balance = WrappedToken::balanceOfCall::abi_decode_returns(&response, true)
            .unwrap()
            ._0;
        Ok(balance.to())
    }

    /// Creates an empty canister with cycles on it's balance.
    async fn create_canister(&self) -> Result<Principal>;

    /// Creates an empty canister with cycles on it's balance and the specified id
    async fn create_canister_with_id(&self, id: Principal) -> Result<Principal>;

    /// Creates an empty canister with cycles on it's balance with the specified id and controller.
    async fn create_canister_with_id_and_controller(
        &self,
        id: Principal,
        controller: Principal,
    ) -> Result<Principal>;

    /// Stop the canister with the given `canister_id`.
    async fn stop_canister(&self, _canister: Principal) -> Result<()> {
        Ok(())
    }

    /// Installs the `wasm` code to the `canister` with the given init `args`.
    async fn install_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()>;

    /// Installs the `wasm` code to the `canister` with the given init `args` and sender.
    async fn install_canister_with_sender(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
        sender: Principal,
    ) -> Result<()>;

    /// Reinstalls the canister.
    async fn reinstall_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()>;

    /// Upgrades the canister.
    async fn upgrade_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()>;

    /// Returns the path to the wasm file for the given canister type.
    async fn get_wasm_path(&self, canister_type: CanisterType) -> PathBuf {
        canister_type.default_canister_wasm_path().await
    }

    /// Installs code to test context's canister with the given type.
    /// If the canister depends on not-created canister, Principal::anonymous() is used.
    async fn install_default_canister(&self, canister_type: CanisterType) {
        let wasm = canister_type.default_canister_wasm().await;
        match canister_type {
            CanisterType::Evm => {
                println!(
                    "Installing EVM Canister with Principal: {}",
                    self.canisters().evm()
                );
                let signature_canister = self.canisters().get_or_anonymous(CanisterType::Signature);
                let evm_principal = self.canisters().evm();

                let init_data = evm_canister_init_data(
                    signature_canister,
                    self.admin(),
                    Some(EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS),
                );
                self.install_canister(evm_principal, wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::ExternalEvm => {
                println!(
                    "Installing default External EVM canister with Principal {}",
                    self.canisters().external_evm()
                );
                let signature_canister = self.canisters().get_or_anonymous(CanisterType::Signature);
                let init_data = evm_canister_init_data(
                    signature_canister,
                    self.admin(),
                    Some(EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS),
                );
                self.install_canister(self.canisters().external_evm(), wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::EvmRpcCanister => {
                println!(
                    "Installing default EVM RPC canister with Principal {}",
                    self.canisters().evm_rpc()
                );
                let init_data = EvmRpcCanisterInitData { nodesInSubnet: 1 };
                self.install_canister(self.canisters().evm_rpc(), wasm, (init_data,))
                    .await
                    .unwrap();

                let client = self.client(self.canisters().evm_rpc(), self.admin_name());

                let res = client
                    .update::<_, bool>(
                        "authorize",
                        (self.admin(), evm_rpc_canister::Auth::RegisterProvider),
                    )
                    .await
                    .expect("authorize failed");
                assert!(res, "authorize failed");

                let EvmLink::Http(hostname) = self.wrapped_evm_link() else {
                    panic!("EVM-RPC provider hostname is not set");
                };
                println!("EVM-RPC provider hostname: {hostname}");
                // configure the EVM RPC canister provider
                let args = evm_rpc_canister::RegisterProviderArgs {
                    chainId: self.wrapped_evm().chain_id().await.unwrap(),
                    hostname,
                    credentialPath: "".to_string(),
                    cyclesPerCall: 1,
                    cyclesPerMessageByte: 1,
                    credentialsHeaders: None,
                };

                client
                    .update::<_, u64>("registerProvider", (args,))
                    .await
                    .expect("registerProvider failed");
            }
            CanisterType::Signature => {
                println!(
                    "Installing default Signature canister with Principal {}",
                    self.canisters().signature_verification()
                );
                let possible_canisters = [CanisterType::Evm, CanisterType::ExternalEvm];
                let init_data = possible_canisters
                    .into_iter()
                    .filter_map(|canister_type| self.canisters().get(canister_type))
                    .collect::<Vec<_>>();

                self.install_canister(
                    self.canisters().signature_verification(),
                    wasm,
                    (init_data,),
                )
                .await
                .unwrap();
            }
            CanisterType::Token1 => {
                println!(
                    "Installing default Token1 canister with Principal {}",
                    self.canisters().token_1()
                );

                let init_balances = self.icrc_token_initial_balances();
                let init_data =
                    icrc_canister_default_init_args(self.admin(), "Tokenium", init_balances);
                self.install_canister(
                    self.canisters().token_1(),
                    wasm,
                    (LedgerArgument::Init(init_data),),
                )
                .await
                .unwrap();
            }
            CanisterType::Token2 => {
                println!(
                    "Installing default Token2 canister with Principal {}",
                    self.canisters().token_2()
                );
                let init_balances = self.icrc_token_initial_balances();
                let init_data =
                    icrc_canister_default_init_args(self.admin(), "Tokenium 2", init_balances);
                self.install_canister(
                    self.canisters().token_2(),
                    wasm,
                    (LedgerArgument::Init(init_data),),
                )
                .await
                .unwrap();
            }
            CanisterType::Icrc2Bridge => {
                println!(
                    "Installing default ICRC2 bridge canister with Principal {}",
                    self.canisters().icrc2_bridge()
                );

                let init_data = icrc_bridge_canister_init_data(
                    self.admin(),
                    self.wrapped_evm_link(),
                    self.sign_key(),
                );
                self.install_canister(self.canisters().icrc2_bridge(), wasm, (init_data,))
                    .await
                    .unwrap();

                // Wait for initialization of the Minter canister parameters.
                self.advance_time(Duration::from_secs(2)).await;
            }
            CanisterType::CkBtcLedger => {
                println!(
                    "Installing default ICRC1 Ledger canister with Principal {}",
                    self.canisters().ckbtc_ledger()
                );
                let init_data = ckbtc_ledger_init_data(self.canisters().ckbtc_minter());
                self.install_canister(self.canisters().ckbtc_ledger(), wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::CkBtcMinter => {
                println!(
                    "Installing default ckBTC minter canister with Principal {}",
                    self.canisters().ckbtc_minter()
                );
                let init_data =
                    ckbtc_minter_init_data(self.canisters().ckbtc_ledger(), self.canisters().kyt());
                self.install_canister(self.canisters().ckbtc_minter(), wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::Bitcoin => {
                println!("Installing default BTC canister...");
                let nns_root_canister = Principal::from_text(NNS_ROOT_CANISTER_ID)
                    .expect("Failed to parse NNS_ROOT_CANISTER_ID");

                let actual_canister_id = self
                    .create_canister_with_id_and_controller(
                        self.canisters().bitcoin(),
                        nns_root_canister,
                    )
                    .await
                    .unwrap();
                assert_eq!(actual_canister_id, self.canisters().bitcoin());

                let btc_wasm = CanisterType::Bitcoin.default_canister_wasm().await;
                let args = ic_btc_interface::Config {
                    network: ic_btc_interface::Network::Regtest,
                    ..Default::default()
                };
                self.install_canister_with_sender(
                    self.canisters().bitcoin(),
                    btc_wasm,
                    (&args,),
                    nns_root_canister,
                )
                .await
                .expect("Failed to install Bitcoin canister");
            }
            CanisterType::Kyt => {
                println!(
                    "Installing default KYT canister with Principal {}",
                    self.canisters().kyt()
                );
                let init_data = kyc_init_data(self.canisters().ckbtc_minter(), self.admin());
                self.install_canister(self.canisters().kyt(), wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::Erc20Bridge => {
                println!(
                    "Installing default CK Erc20 bridge canister with Principal {}",
                    self.canisters().erc20_bridge()
                );
                let init_data = erc20_bridge_canister_init_data(
                    self.admin(),
                    self.wrapped_evm_link(),
                    self.sign_key(),
                );

                let base_evm_settings = BaseEvmSettings {
                    evm_link: self.base_evm_link(),
                    signing_strategy: SigningStrategy::ManagementCanister {
                        key_id: self.sign_key(),
                    },
                    delays: QueryDelays {
                        evm_params_query: Duration::from_secs(2),
                        logs_query: Duration::from_secs(2),
                    },
                };
                self.install_canister(
                    self.canisters().erc20_bridge(),
                    wasm,
                    (init_data, base_evm_settings),
                )
                .await
                .unwrap();
            }
            CanisterType::BtcBridge => {
                println!(
                    "Installing default BTC bridge canister with Principal {}",
                    self.canisters().btc_bridge()
                );
                let init_data = btc_bridge_canister_init_data(
                    self.admin(),
                    self.wrapped_evm_link(),
                    self.sign_key(),
                    self.canisters().ckbtc_minter(),
                    self.canisters().ckbtc_ledger(),
                );

                self.install_canister(self.canisters().btc_bridge(), wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::Brc20Bridge => {
                println!(
                    "Installing default BRC20 bridge canister with Principal {}",
                    self.canisters().brc20_bridge()
                );
                let init_data = brc20_bridge_canister_init_data(
                    self.admin(),
                    self.wrapped_evm_link(),
                    self.sign_key(),
                );

                self.install_canister(self.canisters().brc20_bridge(), wasm, init_data)
                    .await
                    .unwrap();
            }
            CanisterType::RuneBridge => {
                println!(
                    "Installing default Rune bridge canister with Principal {}",
                    self.canisters().rune_bridge()
                );
                let init_data = rune_bridge_canister_init_data(
                    self.admin(),
                    self.wrapped_evm_link(),
                    self.sign_key(),
                );

                self.install_canister(self.canisters().rune_bridge(), wasm, init_data)
                    .await
                    .unwrap();
            }
        }
    }

    /// Upgrades the icrc2 bridge canister with default settings.
    async fn upgrade_icrc2_bridge_canister(&self) -> Result<()> {
        let wasm = get_icrc2_bridge_canister_bytecode().await;
        self.upgrade_canister(self.canisters().icrc2_bridge(), wasm, ())
            .await?;
        Ok(())
    }

    /// Reinstalls the icrc1 token canister with default settings.
    async fn reinstall_icrc1_canister(
        &self,
        token_canister: Principal,
        token_name: &str,
        initial_balances: Vec<(Account, Nat)>,
    ) -> Result<()> {
        let init_args = icrc_canister_default_init_args(self.admin(), token_name, initial_balances);
        let args = LedgerArgument::Init(init_args);
        let wasm = get_icrc1_token_canister_bytecode().await;
        self.reinstall_canister(token_canister, wasm, (args,))
            .await?;

        Ok(())
    }

    async fn reinstall_icrc2_bridge_canister(&self) -> Result<()> {
        eprintln!("reinstalling icrc2 bridge canister");
        let init_data =
            icrc_bridge_canister_init_data(self.admin(), self.wrapped_evm_link(), self.sign_key());

        let wasm = get_icrc2_bridge_canister_bytecode().await;
        self.reinstall_canister(self.canisters().icrc2_bridge(), wasm, (init_data,))
            .await?;

        Ok(())
    }

    fn icrc_token_initial_balances(&self) -> Vec<(Account, Nat)>;
}

pub const ICRC1_TRANSFER_FEE: u64 = 10_000;
pub const ICRC1_INITIAL_BALANCE: u64 = 10u64.pow(18);

pub fn icrc_canister_default_init_args(
    caller: Principal,
    token_name: &str,
    initial_balances: Vec<(Account, Nat)>,
) -> InitArgs {
    InitArgs {
        minting_account: Account::from(caller),
        fee_collector_account: None,
        initial_balances,
        transfer_fee: Nat::from(ICRC1_TRANSFER_FEE),
        token_name: token_name.to_string(),
        token_symbol: "TKN".to_string(),
        metadata: vec![(
            "icrc1:name".to_string(),
            MetadataValue::Text(token_name.to_string()),
        )],
        archive_options: ArchiveOptions {
            trigger_threshold: 10,
            num_blocks_to_archive: 5,
            node_max_memory_size_bytes: None,
            max_message_size_bytes: None,
            controller_id: caller,
            cycles_for_archive_creation: None,
            max_transactions_per_response: None,
        },
        max_memo_length: None,
        feature_flags: Some(FeatureFlags { icrc2: true }),
        decimals: None,
        maximum_number_of_accounts: None,
        accounts_overflow_trim_quantity: None,
    }
}

pub fn icrc_bridge_canister_init_data(
    owner: Principal,
    evm_link: EvmLink,
    key_id: SigningKeyId,
) -> BridgeInitData {
    BridgeInitData {
        owner,
        evm_link,
        signing_strategy: SigningStrategy::ManagementCanister { key_id },
        log_settings: Some(LogCanisterSettings {
            enable_console: Some(true),
            in_memory_records: None,
            log_filter: Some("trace".to_string()),
            ..Default::default()
        }),
    }
}

pub fn erc20_bridge_canister_init_data(
    owner: Principal,
    evm_link: EvmLink,
    key_id: SigningKeyId,
) -> BridgeInitData {
    BridgeInitData {
        owner,
        evm_link,
        signing_strategy: SigningStrategy::ManagementCanister { key_id },
        log_settings: Some(LogCanisterSettings {
            enable_console: Some(true),
            in_memory_records: None,
            log_filter: Some("trace".to_string()),
            ..Default::default()
        }),
    }
}

pub fn btc_bridge_canister_init_data(
    owner: Principal,
    evm_link: EvmLink,
    key_id: SigningKeyId,
    ckbtc_minter: Principal,
    ckbtc_ledger: Principal,
) -> BtcBridgeConfig {
    let init_data = icrc_bridge_canister_init_data(owner, evm_link, key_id);

    BtcBridgeConfig {
        init_data,
        network: BitcoinConnection::Custom {
            network: BitcoinNetwork::Regtest,
            ckbtc_minter,
            ckbtc_ledger,
            ledger_fee: 1_000,
        },
    }
}

pub fn rune_bridge_canister_init_data(
    owner: Principal,
    evm_link: EvmLink,
    key_id: SigningKeyId,
) -> (BridgeInitData, RuneBridgeConfig) {
    let init_data = icrc_bridge_canister_init_data(owner, evm_link, key_id);

    (
        init_data,
        RuneBridgeConfig {
            network: BitcoinNetwork::Regtest,
            btc_cache_timeout_secs: None,
            min_confirmations: 1,
            indexers: vec![IndexerType::OrdHttp {
                url: "http://localhost:8000".to_string(),
            }],
            deposit_fee: 500_000,
            mempool_timeout: Duration::from_secs(60),
            indexer_consensus_threshold: 1,
        },
    )
}

pub fn brc20_bridge_canister_init_data(
    owner: Principal,
    evm_link: EvmLink,
    key_id: SigningKeyId,
) -> (BridgeInitData, Brc20BridgeConfig) {
    let init_data = icrc_bridge_canister_init_data(owner, evm_link, key_id);

    (
        init_data,
        Brc20BridgeConfig {
            network: BitcoinNetwork::Regtest,
            min_confirmations: 1,
            indexer_urls: HashSet::from_iter(["http://localhost:8004".to_string()]),
            deposit_fee: 500_000,
            mempool_timeout: Duration::from_secs(60),
            indexer_consensus_threshold: 1,
            schnorr_key_id: SchnorrKeyIds::TestKeyLocalDevelopment,
        },
    )
}

pub fn evm_canister_init_data(
    signature_verification_principal: Principal,
    owner: Principal,
    transaction_processing_interval: Option<Duration>,
) -> EvmCanisterInitData {
    #[allow(deprecated)]
    EvmCanisterInitData {
        signature_verification_principal,
        min_gas_price: 10_u64.into(),
        chain_id: CHAIN_ID,
        log_settings: Some(ic_log::LogSettings {
            enable_console: true,
            in_memory_records: None,
            log_filter: Some("debug".to_string()),
        }),
        transaction_processing_interval,
        owner,
        ..Default::default()
    }
}

fn ckbtc_ledger_init_data(minter_principal: Principal) -> LedgerArgument {
    let minting_account = Account {
        owner: minter_principal,
        subaccount: None,
    };
    let archive_options = ArchiveOptions {
        trigger_threshold: 10_000_000,
        num_blocks_to_archive: 1_000_000,
        node_max_memory_size_bytes: None,
        max_message_size_bytes: None,
        controller_id: Principal::anonymous(),
        cycles_for_archive_creation: None,
        max_transactions_per_response: None,
    };

    LedgerArgument::Init(InitArgs {
        minting_account,
        fee_collector_account: None,
        initial_balances: vec![],
        transfer_fee: Nat::from(1_000u64),
        decimals: None,
        token_name: "ckBtc".into(),
        token_symbol: "ckBtc".into(),
        metadata: vec![],
        archive_options,
        max_memo_length: Some(80),
        feature_flags: Some(FeatureFlags { icrc2: true }),
        maximum_number_of_accounts: None,
        accounts_overflow_trim_quantity: None,
    })
}

fn ckbtc_minter_init_data(ledger: Principal, kyt: Principal) -> MinterArg {
    MinterArg::Init(crate::utils::btc::InitArgs {
        btc_network: BtcNetwork::Regtest,
        ecdsa_key_name: "dfx_test_key".to_string(),
        retrieve_btc_min_amount: 100_000,
        ledger_id: ledger,
        max_time_in_queue_nanos: 100,
        min_confirmations: Some(1),
        mode: Mode::GeneralAvailability,
        kyt_fee: Some(2000),
        kyt_principal: Some(kyt),
    })
}

fn kyc_init_data(ckbtc_minter: Principal, admin: Principal) -> LifecycleArg {
    LifecycleArg::InitArg(InitArg {
        minter_id: ckbtc_minter,
        maintainers: vec![admin],
        mode: KytMode::AcceptAll,
    })
}

#[derive(Debug, Clone, Default)]
pub struct TestCanisters(HashMap<CanisterType, Principal>);

impl TestCanisters {
    pub fn token_1(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Token1)
            .expect("token_1 canister should be initialized (see `TestContext::new()`)")
    }

    pub fn token_2(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Token2)
            .expect("token_2 canister should be initialized (see `TestContext::new()`)")
    }

    pub fn evm(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Evm)
            .expect("evm canister should be initialized (see `TestContext::new()`)")
    }

    pub fn external_evm(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::ExternalEvm)
            .expect("external evm canister should be initialized (see `TestContext::new()`)")
    }

    pub fn evm_rpc(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::EvmRpcCanister)
            .expect("evm rpc canister should be initialized (see `TestContext::new()`)")
    }

    pub fn signature_verification(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Signature)
            .expect("signature canister should be initialized (see `TestContext::new()`)")
    }

    pub fn icrc2_bridge(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Icrc2Bridge)
            .expect("icrc2 bridge canister should be initialized (see `TestContext::new()`)")
    }

    pub fn erc20_bridge(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Erc20Bridge)
            .expect("erc20 bridge canister should be initialized (see `TestContext::new()`)")
    }

    pub fn ckbtc_minter(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::CkBtcMinter)
            .expect("ckBTC minter canister should be initialized (see `TestContext::new()`)")
    }

    pub fn bitcoin(&self) -> Principal {
        Principal::from_text(BITCOIN_CANISTER_ID).expect("bitcoin canister id is invalid")
    }

    pub fn ckbtc_ledger(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::CkBtcLedger)
            .expect("icrc1 ledger canister should be initialized (see `TestContext::new()`)")
    }

    pub fn kyt(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Kyt)
            .expect("kyt canister should be initialized (see `TestContext::new()`)")
    }

    pub fn btc_bridge(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::BtcBridge)
            .expect("bridge canister should be initialized (see `TestContext::new()`)")
    }

    pub fn rune_bridge(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::RuneBridge)
            .expect("rune bridge canister should be initialized (see `TestContext::new()`)")
    }

    pub fn brc20_bridge(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Brc20Bridge)
            .expect("brc20 bridge canister should be initialized (see `TestContext::new()`)")
    }

    pub fn set(&mut self, canister_type: CanisterType, principal: Principal) {
        self.0.insert(canister_type, principal);
    }

    pub fn get(&self, canister_type: CanisterType) -> Option<Principal> {
        self.0.get(&canister_type).copied()
    }

    pub fn get_or_anonymous(&self, canister_type: CanisterType) -> Principal {
        self.0
            .get(&canister_type)
            .copied()
            .unwrap_or_else(Principal::anonymous)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanisterType {
    Bitcoin,
    Brc20Bridge,
    BtcBridge,
    CkBtcMinter,
    Erc20Bridge,
    Evm,
    EvmRpcCanister,
    ExternalEvm,
    CkBtcLedger,
    Icrc2Bridge,
    Kyt,
    RuneBridge,
    Signature,
    Token1,
    Token2,
}

impl CanisterType {
    /// EVM, SignatureVerification, Icrc2Bridge and Token1.
    pub const ICRC2_MINTER_TEST_SET: [CanisterType; 2] =
        [CanisterType::Token1, CanisterType::Icrc2Bridge];

    /// EVM, ExternalEvm, SignatureVerification, Erc20Bridge
    pub const EVM_MINTER_TEST_SET: [CanisterType; 1] = [CanisterType::Erc20Bridge];

    /// EVM, ExternalEvm, EvmRpc, SignatureVerification, Erc20Bridge
    pub const EVM_MINTER_WITH_EVMRPC_TEST_SET: [CanisterType; 2] =
        [CanisterType::EvmRpcCanister, CanisterType::Erc20Bridge];

    pub const BTC_CANISTER_SET: [CanisterType; 5] = [
        CanisterType::Bitcoin,
        CanisterType::BtcBridge,
        CanisterType::CkBtcMinter,
        CanisterType::Kyt,
        CanisterType::CkBtcLedger, // acts as CkBtcLedger
    ];

    pub const BRC20_CANISTER_SET: [CanisterType; 1] = [CanisterType::Brc20Bridge];

    pub const RUNE_CANISTER_SET: [CanisterType; 1] = [CanisterType::RuneBridge];

    pub async fn default_canister_wasm(&self) -> Vec<u8> {
        match self {
            CanisterType::Brc20Bridge => get_brc20_bridge_canister_bytecode().await,
            CanisterType::Bitcoin => get_ic_btc_canister_bytecode().await,
            CanisterType::BtcBridge => get_btc_bridge_canister_bytecode().await,
            CanisterType::CkBtcMinter => get_ckbtc_minter_canister_bytecode().await,
            CanisterType::Erc20Bridge => get_ck_erc20_bridge_canister_bytecode().await,
            CanisterType::Evm => get_evm_testnet_canister_bytecode().await,
            CanisterType::EvmRpcCanister => get_evm_rpc_canister_bytecode().await,
            CanisterType::ExternalEvm => get_evm_testnet_canister_bytecode().await,
            CanisterType::CkBtcLedger => get_icrc1_token_canister_bytecode().await,
            CanisterType::Icrc2Bridge => get_icrc2_bridge_canister_bytecode().await,
            CanisterType::Kyt => get_kyt_canister_bytecode().await,
            CanisterType::RuneBridge => get_rune_bridge_canister_bytecode().await,
            CanisterType::Signature => get_signature_verification_canister_bytecode().await,
            CanisterType::Token1 => get_icrc1_token_canister_bytecode().await,
            CanisterType::Token2 => get_icrc1_token_canister_bytecode().await,
        }
    }

    pub async fn default_canister_wasm_path(&self) -> PathBuf {
        match self {
            CanisterType::Brc20Bridge => get_brc20_bridge_canister_wasm_path().await,
            CanisterType::Bitcoin => get_ic_btc_canister_wasm_path().await,
            CanisterType::BtcBridge => get_btc_bridge_canister_wasm_path().await,
            CanisterType::CkBtcMinter => get_ckbtc_minter_canister_wasm_path().await,
            CanisterType::Erc20Bridge => get_ck_erc20_bridge_canister_wasm_path().await,
            CanisterType::Evm => get_evm_testnet_canister_wasm_path().await,
            CanisterType::EvmRpcCanister => get_evm_rpc_canister_wasm_path().await,
            CanisterType::ExternalEvm => get_evm_testnet_canister_wasm_path().await,
            CanisterType::CkBtcLedger => get_icrc1_token_canister_wasm_path().await,
            CanisterType::Icrc2Bridge => get_icrc2_bridge_canister_wasm_path().await,
            CanisterType::Kyt => get_kyt_canister_wasm_path().await,
            CanisterType::RuneBridge => get_rune_bridge_canister_wasm_path().await,
            CanisterType::Signature => get_signature_verification_canister_wasm_path().await,
            CanisterType::Token1 => get_icrc1_token_canister_wasm_path().await,
            CanisterType::Token2 => get_icrc1_token_canister_wasm_path().await,
        }
    }
}
