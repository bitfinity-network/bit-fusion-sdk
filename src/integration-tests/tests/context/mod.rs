use std::collections::HashMap;
use std::time::Duration;

use candid::utils::ArgumentEncoder;
use candid::{Nat, Principal};
use did::constant::EIP1559_INITIAL_BASE_FEE;
use did::error::EvmError;
use did::init::EvmCanisterInitData;
use did::{NotificationInput, Transaction, TransactionReceipt, H160, H256, U256, U64};
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use eth_signer::{Signer, Wallet};
use ethers_core::abi::Token;
use ethers_core::k256::ecdsa::SigningKey;
use evm_canister_client::{CanisterClient, EvmCanisterClient};
use ic_exports::ic_kit::mock_principals::alice;
use ic_exports::icrc_types::icrc::generic_metadata_value::MetadataValue;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_exports::icrc_types::icrc1_ledger::{
    ArchiveOptions, FeatureFlags, InitArgs, LedgerArgument,
};
use ic_log::LogSettings;
use icrc2_minter::SigningStrategy;
use minter_client::MinterCanisterClient;
use minter_contract_utils::build_data::test_contracts::BFT_BRIDGE_SMART_CONTRACT_CODE;
use minter_contract_utils::{bft_bridge_api, wrapped_token_api};
use minter_did::error::Result as McResult;
use minter_did::id256::Id256;
use minter_did::init::InitData;
use minter_did::order::SignedMintOrder;
use minter_did::reason::Icrc2Burn;
use tokio::time::Instant;

use super::utils::error::Result;
use crate::utils::btc::{BtcNetwork, InitArg, KytMode, LifecycleArg, MinterArg, Mode};
use crate::utils::error::TestError;
use crate::utils::icrc_client::IcrcClient;
use crate::utils::wasm::{
    get_btc_bridge_canister_bytecode, get_btc_canister_bytecode,
    get_ck_btc_minter_canister_bytecode, get_erc20_minter_canister_bytecode,
    get_evm_testnet_canister_bytecode, get_icrc1_token_canister_bytecode,
    get_kyt_canister_bytecode, get_minter_canister_bytecode,
    get_signature_verification_canister_bytecode, get_spender_canister_bytecode,
};
use crate::utils::{CHAIN_ID, EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS};

pub const DEFAULT_GAS_PRICE: u128 = EIP1559_INITIAL_BASE_FEE * 2;

#[async_trait::async_trait]
pub trait TestContext {
    type Client: CanisterClient + Send + Sync;

    /// Returns principals for cansters in the context.
    fn canisters(&self) -> TestCanisters;

    /// Returns client for the canister.
    fn client(&self, canister: Principal, caller: &str) -> Self::Client;

    /// Principal to use for canisters initialization.
    fn admin(&self) -> Principal;

    /// Principal to use for canisters initialization.
    fn admin_name(&self) -> &str;

    /// Returns client for the evm canister.
    fn evm_client(&self, caller: &str) -> EvmCanisterClient<Self::Client> {
        EvmCanisterClient::new(self.client(self.canisters().evm(), caller))
    }

    /// Returns client for the evm canister.
    fn minter_client(&self, caller: &str) -> MinterCanisterClient<Self::Client> {
        MinterCanisterClient::new(self.client(self.canisters().minter(), caller))
    }

    /// Returns client for the ICRC token 1 canister.
    fn icrc_token_1_client(&self, caller: &str) -> IcrcClient<Self::Client> {
        IcrcClient::new(self.client(self.canisters().token_1(), caller))
    }

    /// Returns client for the ICRC token 2 canister.
    fn icrc_token_2_client(&self, caller: &str) -> IcrcClient<Self::Client> {
        IcrcClient::new(self.client(self.canisters().token_2(), caller))
    }

    /// Sends tx with notification to EVMc.
    async fn send_notification_tx(
        &self,
        user: &Wallet<SigningKey>,
        input: NotificationInput,
    ) -> Result<H256> {
        let address: H160 = user.address().into();
        let client = self.evm_client(self.admin_name());
        let account = client.account_basic(address.clone()).await?;

        let tx = self.signed_transaction(
            user,
            Some(address.clone()),
            account.nonce,
            0,
            input.encode().unwrap(),
        );

        Ok(client.send_raw_transaction(tx).await??)
    }

    /// Waits for transaction receipt.
    async fn wait_transaction_receipt(&self, hash: &H256) -> Result<Option<TransactionReceipt>> {
        let client = self.evm_client(self.admin_name());
        self.wait_transaction_receipt_on_evm(&client, hash).await
    }

    /// Waits for transaction receipt.
    async fn wait_transaction_receipt_on_evm(
        &self,
        evm_client: &EvmCanisterClient<Self::Client>,
        hash: &H256,
    ) -> Result<Option<TransactionReceipt>> {
        let tx_processing_interval = EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS;
        let timeout = tx_processing_interval * 2;
        let start = Instant::now();
        let mut time_passed = Duration::ZERO;
        let mut receipt = None;
        while time_passed < timeout && receipt.is_none() {
            self.advance_time(tx_processing_interval).await;
            time_passed = Instant::now() - start;
            receipt = evm_client
                .eth_get_transaction_receipt(hash.clone())
                .await??;
        }
        Ok(receipt)
    }

    async fn advance_time(&self, time: Duration);

    /// Creates a new wallet with the EVM balance on it.
    async fn new_wallet(&self, balance: u128) -> Result<Wallet<'static, SigningKey>> {
        let wallet = {
            let mut rng = rand::thread_rng();
            Wallet::new(&mut rng)
        };
        let client = self.evm_client(self.admin_name());
        client
            .mint_native_tokens(wallet.address().into(), balance.into())
            .await??;

        self.advance_time(Duration::from_secs(2)).await;

        Ok(wallet)
    }

    /// Returns minter canister EVM address.
    async fn get_minter_canister_evm_address(&self, caller: &str) -> Result<H160> {
        let client = self.client(self.canisters().minter(), caller);
        Ok(client
            .update::<_, McResult<H160>>("get_minter_canister_evm_address", ())
            .await??)
    }

    /// Creates contract in EVMc.
    async fn create_contract(
        &self,
        creator_wallet: &Wallet<'_, SigningKey>,
        input: Vec<u8>,
    ) -> Result<H160> {
        let evm_client = self.evm_client(self.admin_name());
        let creator_address: H160 = creator_wallet.address().into();
        let nonce = evm_client
            .account_basic(creator_address.clone())
            .await
            .unwrap()
            .nonce;

        let create_contract_tx = self.signed_transaction(creator_wallet, None, nonce, 0, input);

        let hash = evm_client
            .send_raw_transaction(create_contract_tx)
            .await??;
        let receipt = self
            .wait_transaction_receipt(&hash)
            .await?
            .ok_or(TestError::Evm(EvmError::Internal(
                "transction not processed".into(),
            )))?;

        if receipt.status != Some(U64::one()) {
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

    /// Crates BFTBridge contract in EVMc and registered it in minter canister
    async fn initialize_bft_bridge(
        &self,
        caller: &str,
        wallet: &Wallet<'_, SigningKey>,
    ) -> Result<H160> {
        let minter_canister_address = self.get_minter_canister_evm_address(caller).await?;

        let client = self.evm_client(self.admin_name());
        client
            .mint_native_tokens(minter_canister_address.clone(), u64::MAX.into())
            .await??;
        self.advance_time(Duration::from_secs(2)).await;

        let minter_client = self.minter_client(caller);

        let contract = BFT_BRIDGE_SMART_CONTRACT_CODE.clone();
        let input = bft_bridge_api::CONSTRUCTOR
            .encode_input(contract, &[Token::Address(minter_canister_address.into())])
            .unwrap();

        let bridge_address = self.create_contract(wallet, input.clone()).await.unwrap();
        minter_client
            .register_evmc_bft_bridge(bridge_address.clone())
            .await??;

        Ok(bridge_address)
    }

    async fn initialize_bft_bridge_with_minter(
        &self,
        wallet: &Wallet<'_, SigningKey>,
        minter_canister_address: H160,
    ) -> Result<H160> {
        let contract = BFT_BRIDGE_SMART_CONTRACT_CODE.clone();
        let input = bft_bridge_api::CONSTRUCTOR
            .encode_input(contract, &[Token::Address(minter_canister_address.into())])
            .unwrap();

        let bridge_address = self.create_contract(wallet, input.clone()).await.unwrap();

        Ok(bridge_address)
    }

    async fn burn_erc_20_tokens_raw(
        &self,
        wallet: &Wallet<'_, SigningKey>,
        from_token: &H160,
        recipient: Vec<u8>,
        bridge: &H160,
        amount: u128,
    ) -> Result<(u32, H256)> {
        let amount = amount.into();
        let input = wrapped_token_api::ERC_20_APPROVE
            .encode_input(&[Token::Address(bridge.0), Token::Uint(amount)])
            .unwrap();

        let results = self.call_contract(wallet, from_token, input, 0).await?;
        let output = results.1.output.unwrap();
        assert_eq!(
            wrapped_token_api::ERC_20_APPROVE
                .decode_output(&output)
                .unwrap()[0],
            Token::Bool(true)
        );

        println!("burning src tokens using BftBridge");
        let input = bft_bridge_api::BURN
            .encode_input(&[
                Token::Uint(amount),
                Token::Address(from_token.0),
                Token::Bytes(recipient),
            ])
            .unwrap();

        let (tx_hash, receipt) = self.call_contract(wallet, bridge, input, 0).await?;
        let decoded_output = bft_bridge_api::BURN
            .decode_output(receipt.output.as_ref().unwrap())
            .unwrap();
        if receipt.status != Some(U64::one()) {
            return Err(TestError::Generic(format!(
                "Burn transaction failed: {decoded_output:?} -- {receipt:?}, -- {}",
                String::from_utf8_lossy(receipt.output.as_ref().unwrap())
            )));
        }

        let operation_id = decoded_output[0].clone().into_uint().unwrap().as_u32();
        Ok((operation_id, tx_hash))
    }

    async fn burn_erc_20_tokens(
        &self,
        wallet: &Wallet<'_, SigningKey>,
        from_token: &H160,
        recipient: Id256,
        bridge: &H160,
        amount: u128,
    ) -> Result<(u32, H256)> {
        self.burn_erc_20_tokens_raw(wallet, from_token, recipient.0.to_vec(), bridge, amount)
            .await
    }

    /// Returns a signed transaction from the given `wallet`.
    fn signed_transaction(
        &self,
        wallet: &Wallet<SigningKey>,
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
            gas: 3_000_000u64.into(),
            gas_price: Some(DEFAULT_GAS_PRICE.into()),
            input,
            signature: SigningMethod::SigningKey(wallet.signer()),
            chain_id: CHAIN_ID,
        }
        .calculate_hash_and_build()
        .unwrap()
    }

    /// Calls contract in EVMc.
    async fn call_contract(
        &self,
        wallet: &Wallet<'_, SigningKey>,
        contract: &H160,
        input: Vec<u8>,
        amount: u128,
    ) -> Result<(H256, TransactionReceipt)> {
        let evm_client = self.evm_client(self.admin_name());
        let from: H160 = wallet.address().into();
        let nonce = evm_client.account_basic(from.clone()).await?.nonce;

        let call_tx = self.signed_transaction(wallet, Some(contract.clone()), nonce, amount, input);

        let hash = evm_client.send_raw_transaction(call_tx).await??;
        let receipt = self
            .wait_transaction_receipt(&hash)
            .await?
            .ok_or(TestError::Evm(EvmError::Internal(
                "transaction not processed".into(),
            )))?;

        if receipt.status != Some(U64::one()) {
            println!("tx status: {:?}", receipt.status);
            dbg!(&receipt);
            dbg!(&hex::encode(receipt.output.as_ref().unwrap_or(&vec![])));
        }

        Ok((hash, receipt))
    }

    /// Creates wrapped token in EVMc by calling `BFTBridge:::deploy_wrapped_token()`.
    async fn create_wrapped_token(
        &self,
        wallet: &Wallet<'_, SigningKey>,
        bft_bridge: &H160,
        base_token_id: Id256,
    ) -> Result<H160> {
        let input = bft_bridge_api::DEPLOY_WRAPPED_TOKEN
            .encode_input(&[
                Token::String("wrapper".into()),
                Token::String("WPT".into()),
                Token::FixedBytes(base_token_id.0.to_vec()),
            ])
            .unwrap();

        let results = self.call_contract(wallet, bft_bridge, input, 0).await?;
        let output = results.1.output.unwrap();

        Ok(bft_bridge_api::DEPLOY_WRAPPED_TOKEN
            .decode_output(&output)
            .unwrap()[0]
            .clone()
            .into_address()
            .unwrap()
            .into())
    }

    /// Burns ICRC-2 token 1 and creates according mint order.
    async fn burn_icrc2(
        &self,
        caller: &str,
        wallet: &Wallet<'_, SigningKey>,
        amount: u128,
        operation_id: u32,
        approve_spender: H160,
        approve_amount: U256,
    ) -> Result<u32> {
        self.approve_icrc2_burn(caller, amount + ICRC1_TRANSFER_FEE as u128)
            .await?;

        let reason = Icrc2Burn {
            amount: amount.into(),
            from_subaccount: None,
            icrc2_token_principal: self.canisters().token_1(),
            recipient_address: wallet.address().into(),
            operation_id,
            approve_spender,
            approve_amount,
        };

        Ok(self.minter_client(caller).burn_icrc2(reason).await??)
    }

    /// Approves burning of ICRC-2 token.
    async fn approve_icrc2_burn(&self, caller: &str, amount: u128) -> Result<()> {
        let client = self.icrc_token_1_client(caller);
        let minter_canister = self.canisters().minter().into();
        client.icrc2_approve(minter_canister, amount.into()).await?;
        Ok(())
    }

    /// Mints ERC-20 token with the order.
    async fn mint_erc_20_with_order(
        &self,
        wallet: &Wallet<'_, SigningKey>,
        bridge: &H160,
        order: SignedMintOrder,
    ) -> Result<TransactionReceipt> {
        let input = bft_bridge_api::MINT
            .encode_input(&[Token::Bytes(order.0.to_vec())])
            .unwrap();
        self.call_contract(wallet, bridge, input, 0)
            .await
            .map(|(_, receipt)| receipt)
    }

    /// Returns ERC-20 balance.
    async fn check_erc20_balance(
        &self,
        token: &H160,
        wallet: &Wallet<'_, SigningKey>,
    ) -> Result<u128> {
        let input = wrapped_token_api::ERC_20_BALANCE
            .encode_input(&[Token::Address(wallet.address())])
            .unwrap();
        let results = self.call_contract(wallet, token, input, 0).await?;
        let output = results.1.output.unwrap();

        Ok(wrapped_token_api::ERC_20_BALANCE
            .decode_output(&output)
            .unwrap()[0]
            .clone()
            .into_uint()
            .unwrap()
            .as_u128())
    }

    /// Creates an empty canister with cycles on it's balance.
    async fn create_canister(&self) -> Result<Principal>;

    async fn create_canister_with_id(&self, id: Principal) -> Result<Principal>;

    /// Installs the `wasm` code to the `canister` with the given init `args`.
    async fn install_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
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

    /// Installs code to test context's canister with the given type.
    /// If the canister depends on not-created canister, Principal::anonimous() is used.
    async fn install_default_canister(&self, canister_type: CanisterType) {
        let wasm = canister_type.default_canister_wasm().await;
        match canister_type {
            CanisterType::Evm => {
                println!("Installing default EVM canister...");
                let signature_canister = self.canisters().get_or_anonymous(CanisterType::Signature);
                let init_data = evm_canister_init_data(
                    signature_canister,
                    self.admin(),
                    Some(EVM_PROCESSING_TRANSACTION_INTERVAL_FOR_TESTS),
                );
                self.install_canister(self.canisters().evm(), wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::Signature => {
                println!("Installing default Signature canister...");
                let evm_canister = self.canisters().evm();
                let init_data = vec![evm_canister];
                self.install_canister(
                    self.canisters().signature_verification(),
                    wasm,
                    (init_data,),
                )
                .await
                .unwrap();
            }
            CanisterType::Token1 => {
                println!("Installing default Token1 canister...");
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
                println!("Installing default Token2 canister...");
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
            CanisterType::Minter => {
                println!("Installing default Minter canister...");
                let evm_canister = self.canisters().get_or_anonymous(CanisterType::Evm);
                let spender_canister = self.canisters().get_or_anonymous(CanisterType::Spender);
                let init_data =
                    minter_canister_init_data(self.admin(), evm_canister, spender_canister);
                self.install_canister(self.canisters().minter(), wasm, (init_data,))
                    .await
                    .unwrap();

                // Wait for initialization of the Minter canister parameters.
                self.advance_time(Duration::from_secs(2)).await;
            }
            CanisterType::Spender => {
                println!("Installing default Spender canister...");
                let minter_canister = self.canisters().get_or_anonymous(CanisterType::Minter);
                let init_data = minter_canister;
                self.install_canister(self.canisters().spender(), wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::Icrc1Ledger => {
                println!("Installing default ICRC1 ledger canister...");
                let init_data = icrc1_ledger_init_data(self.canisters().ck_btc_minter());
                self.install_canister(self.canisters().icrc1_ledger(), wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::CkBtcMinter => {
                println!("Installing default ckBTC minter canister...");
                let init_data = ck_btc_minter_init_data(
                    self.canisters().icrc1_ledger(),
                    self.canisters().kyt(),
                );
                self.install_canister(self.canisters().ck_btc_minter(), wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::Btc => {
                println!("Installing default mock ckBTC canister...");
                todo!()
            }
            CanisterType::Kyt => {
                println!("Installing default KYT canister...");
                let init_data = kyc_init_data(self.canisters().ck_btc_minter());
                self.install_canister(self.canisters().kyt(), wasm, (init_data,))
                    .await
                    .unwrap();
            }
            CanisterType::EvmMinter => {
                todo!()
            }
            CanisterType::BtcBridge => {
                todo!()
            }
        }
    }

    /// Reinstall the EVM canister with default settings.
    async fn reinstall_evm_canister(
        &self,
        transaction_processing_interval: Option<Duration>,
    ) -> Result<()> {
        let init_data = evm_canister_init_data(
            self.canisters().signature_verification(),
            self.admin(),
            transaction_processing_interval,
        );
        let wasm = get_evm_testnet_canister_bytecode().await;
        self.reinstall_canister(self.canisters().evm(), wasm, (init_data,))
            .await?;

        Ok(())
    }

    /// Upgrades the EVM canister with default settings.
    async fn upgrade_evm_canister(&self) -> Result<()> {
        let wasm = get_evm_testnet_canister_bytecode().await;
        self.upgrade_canister(self.canisters().evm(), wasm, ())
            .await?;
        Ok(())
    }

    /// Upgrades the minter canister with default settings.
    async fn upgrade_minter_canister(&self) -> Result<()> {
        let wasm = get_minter_canister_bytecode().await;
        self.upgrade_canister(self.canisters().minter(), wasm, ())
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

    async fn reinstall_minter_canister(&self) -> Result<()> {
        eprintln!("reinstalling Minter canister");
        let init_data = minter_canister_init_data(
            self.admin(),
            self.canisters().evm(),
            self.canisters().spender(),
        );

        let wasm = get_minter_canister_bytecode().await;
        self.reinstall_canister(self.canisters().minter(), wasm, (init_data,))
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

pub fn minter_canister_init_data(
    owner: Principal,
    evm_principal: Principal,
    spender_principal: Principal,
) -> InitData {
    let mut rng = rand::thread_rng();
    let wallet = Wallet::new(&mut rng);
    InitData {
        owner,
        evm_principal,
        spender_principal,
        signing_strategy: SigningStrategy::Local {
            private_key: wallet.signer().to_bytes().into(),
        },
        log_settings: Some(LogSettings {
            enable_console: true,
            in_memory_records: None,
            log_filter: Some("trace".to_string()),
        }),
    }
}

pub fn evm_canister_init_data(
    signature_verification_principal: Principal,
    owner: Principal,
    transaction_processing_interval: Option<Duration>,
) -> EvmCanisterInitData {
    EvmCanisterInitData {
        signature_verification_principal,
        min_gas_price: 10_u64.into(),
        chain_id: CHAIN_ID,
        log_settings: Some(LogSettings {
            enable_console: true,
            in_memory_records: None,
            log_filter: Some("debug".to_string()),
        }),
        transaction_processing_interval,
        owner,
        ..Default::default()
    }
}

fn icrc1_ledger_init_data(minter_principal: Principal) -> LedgerArgument {
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
        transfer_fee: Nat::from(10u32),
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

fn ck_btc_minter_init_data(ledger: Principal, kyt: Principal) -> MinterArg {
    MinterArg::Init(crate::utils::btc::InitArgs {
        btc_network: BtcNetwork::Mainnet,
        ecdsa_key_name: "master_ecdsa_public_key_fscpm-uiaaa-aaaaa-aaaap-yai".to_string(),
        retrieve_btc_min_amount: 100_000,
        ledger_id: ledger,
        max_time_in_queue_nanos: 100,
        min_confirmations: Some(12),
        mode: Mode::GeneralAvailability,
        kyt_fee: Some(2000),
        kyt_principal: Some(kyt),
    })
}

fn kyc_init_data(ck_btc_minter: Principal) -> LifecycleArg {
    LifecycleArg::InitArg(InitArg {
        minter_id: ck_btc_minter,
        maintainers: vec![alice()],
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

    pub fn signature_verification(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Signature)
            .expect("signature canister should be initialized (see `TestContext::new()`)")
    }

    pub fn spender(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Spender)
            .expect("spender canister should be initialized (see `TestContext::new()`)")
    }

    pub fn minter(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Minter)
            .expect("minter canister should be initialized (see `TestContext::new()`)")
    }

    pub fn ck_btc_minter(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::CkBtcMinter)
            .expect("ckBTC minter canister should be initialized (see `TestContext::new()`)")
    }

    pub fn btc_mock(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Btc)
            .expect("bitcoin mock canister should be initialized (see `TestContext::new()`)")
    }

    pub fn icrc1_ledger(&self) -> Principal {
        *self
            .0
            .get(&CanisterType::Icrc1Ledger)
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

    pub fn set(&mut self, canister_type: CanisterType, principal: Principal) {
        self.0.insert(canister_type, principal);
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
    Evm,
    Signature,
    Token1,
    Token2,
    Minter,
    Spender,
    EvmMinter,
    Btc,
    CkBtcMinter,
    Kyt,
    Icrc1Ledger,
    BtcBridge,
}

impl CanisterType {
    /// EVM and SignatureVerification.
    pub const EVM_TEST_SET: [CanisterType; 2] = [CanisterType::Evm, CanisterType::Signature];

    /// EVM, SignatureVerification, Minter, Spender and Token1.
    pub const MINTER_TEST_SET: [CanisterType; 5] = [
        CanisterType::Evm,
        CanisterType::Signature,
        CanisterType::Token1,
        CanisterType::Minter,
        CanisterType::Spender,
    ];

    pub const BTC_CANISTER_SET: [CanisterType; 4] = [
        CanisterType::Btc,
        CanisterType::CkBtcMinter,
        CanisterType::Kyt,
        CanisterType::Icrc1Ledger,
    ];

    pub async fn default_canister_wasm(&self) -> Vec<u8> {
        match self {
            CanisterType::Evm => get_evm_testnet_canister_bytecode().await,
            CanisterType::Signature => get_signature_verification_canister_bytecode().await,
            CanisterType::Token1 => get_icrc1_token_canister_bytecode().await,
            CanisterType::Token2 => get_icrc1_token_canister_bytecode().await,
            CanisterType::Minter => get_minter_canister_bytecode().await,
            CanisterType::Spender => get_spender_canister_bytecode().await,
            CanisterType::EvmMinter => get_erc20_minter_canister_bytecode().await,
            CanisterType::Btc => get_btc_canister_bytecode().await,
            CanisterType::CkBtcMinter => get_ck_btc_minter_canister_bytecode().await,
            CanisterType::Kyt => get_kyt_canister_bytecode().await,
            CanisterType::Icrc1Ledger => get_icrc1_token_canister_bytecode().await,
            CanisterType::BtcBridge => get_btc_bridge_canister_bytecode().await,
        }
    }
}
