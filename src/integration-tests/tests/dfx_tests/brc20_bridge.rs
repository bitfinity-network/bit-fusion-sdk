use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::Duration;

use alloy_sol_types::SolCall;
use bitcoin::key::Secp256k1;
use bitcoin::{Address, Amount, PrivateKey, Txid};
use brc20_bridge::brc20_info::{Brc20Info, Brc20Tick};
use brc20_bridge::interface::{DepositError, GetAddressError};
use brc20_bridge::ops::{Brc20BridgeOp, Brc20DepositRequestData};
use brc20_bridge::state::Brc20BridgeConfig;
use bridge_client::BridgeCanisterClient;
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_utils::bft_events::MinterNotificationType;
use bridge_utils::BFTBridge;
use candid::{Encode, Principal};
use did::constant::EIP1559_INITIAL_BASE_FEE;
use did::{BlockNumber, TransactionReceipt, H160, H256};
use eth_signer::sign_strategy::{SigningKeyId, SigningStrategy};
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use ic_canister_client::CanisterClient;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_log::did::LogCanisterSettings;
use ord_rs::Utxo;
use tokio::time::Instant;

use crate::context::{CanisterType, TestContext};
use crate::dfx_tests::{DfxTestContext, ADMIN};
use crate::utils::btc_rpc_client::BitcoinRpcClient;
use crate::utils::wasm::get_brc20_bridge_canister_bytecode;

struct Brc20Wallet {
    admin_address: Address,
    admin_btc_rpc_client: BitcoinRpcClient,
    ord_wallet: BtcWallet,
    brc20_tokens: HashSet<Brc20Tick>,
}

struct BtcWallet {
    private_key: PrivateKey,
    address: Address,
}

fn generate_btc_wallet() -> BtcWallet {
    use rand::Rng as _;
    let entropy = rand::thread_rng().gen::<[u8; 16]>();
    let mnemonic = bip39::Mnemonic::from_entropy(&entropy).unwrap();

    let seed = mnemonic.to_seed("");

    let private_key =
        bitcoin::PrivateKey::from_slice(&seed[..32], bitcoin::Network::Regtest).unwrap();
    let public_key = private_key.public_key(&Secp256k1::new());

    let address = Address::p2wpkh(&public_key, bitcoin::Network::Regtest).unwrap();

    BtcWallet {
        private_key,
        address,
    }
}

fn generate_brc20_tick() -> Brc20Tick {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let mut name = String::new();
    for _ in 0..4 {
        name.push(rng.gen_range(b'a'..=b'z') as char);
    }
    let tick = Brc20Tick::from_str(&name).unwrap();

    tick
}

fn generate_wallet_name() -> String {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let mut name = String::new();
    for _ in 0..12 {
        name.push(rng.gen_range(b'a'..=b'z') as char);
    }

    name
}

struct Brc20Context {
    inner: DfxTestContext,
    eth_wallet: Wallet<'static, SigningKey>,
    bft_bridge_contract: H160,
    brc20: Brc20Wallet,
    tokens: HashMap<Brc20Tick, H160>,
}

/// Setup a new rune for DFX tests
async fn dfx_brc20_setup(brc20_to_deploy: &[Brc20Tick]) -> anyhow::Result<Brc20Wallet> {
    let wallet_name = generate_wallet_name();
    let admin_btc_rpc_client = BitcoinRpcClient::dfx_test_client(&wallet_name);
    let admin_address = admin_btc_rpc_client.get_new_address()?;

    admin_btc_rpc_client.generate_to_address(&admin_address, 101)?;

    // create ord wallet
    let ord_wallet = generate_btc_wallet();

    let mut brc20_tokens = HashSet::new();

    for brc20 in brc20_to_deploy {
        let commit_fund_tx =
            admin_btc_rpc_client.send_to_address(&ord_wallet.address, Amount::from_int_btc(10))?;
        admin_btc_rpc_client.generate_to_address(&admin_address, 1)?;

        let commit_utxo =
            admin_btc_rpc_client.get_utxo_by_address(&commit_fund_tx, &ord_wallet.address)?;

        // deploy
        todo!();
        // mint
        todo!();

        brc20_tokens.insert(*brc20);
    }

    Ok(Brc20Wallet {
        brc20_tokens,
        admin_btc_rpc_client,
        admin_address,
        ord_wallet,
    })
}

impl Brc20Context {
    async fn new(brc20_to_deploy: &[Brc20Tick]) -> Self {
        let brc20_wallet = dfx_brc20_setup(brc20_to_deploy)
            .await
            .expect("failed to setup brc20 tokens");

        let context = DfxTestContext::new(&CanisterType::RUNE_CANISTER_SET).await;
        context
            .evm_client(ADMIN)
            .set_logger_filter("info")
            .await
            .expect("failed to set logger filter")
            .unwrap();

        let bridge = context.canisters().brc20_bridge();
        let init_args = Brc20BridgeConfig {
            network: BitcoinNetwork::Regtest,
            evm_principal: context.canisters().evm(),
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: SigningKeyId::Dfx,
            },
            admin: context.admin(),
            log_settings: LogCanisterSettings {
                enable_console: Some(true),
                in_memory_records: None,
                log_filter: Some("trace".to_string()),
                ..Default::default()
            },
            min_confirmations: 1,
            no_of_indexers: 1,
            indexer_urls: HashSet::from_iter(["https://localhost:8005".to_string()]),
            deposit_fee: 500_000,
            mempool_timeout: Duration::from_secs(60),
        };
        context
            .install_canister(
                bridge,
                get_brc20_bridge_canister_bytecode().await,
                (init_args,),
            )
            .await
            .unwrap();
        let _: () = context
            .client(bridge, ADMIN)
            .update("admin_configure_ecdsa", ())
            .await
            .unwrap();

        let wallet = context.new_wallet(u128::MAX).await.unwrap();

        let btc_bridge_eth_address = context
            .rune_bridge_client(ADMIN)
            .get_bridge_canister_evm_address()
            .await
            .unwrap();

        let client = context.evm_client(ADMIN);
        client
            .mint_native_tokens(btc_bridge_eth_address.clone().unwrap(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();

        let bft_bridge = context
            .initialize_bft_bridge_with_minter(&wallet, btc_bridge_eth_address.unwrap(), None, true)
            .await
            .unwrap();

        let mut tokens = HashMap::new();

        for brc20_token in &brc20_wallet.brc20_tokens {
            let token = context
                .create_wrapped_token(&wallet, &bft_bridge, (*brc20_token).into())
                .await
                .unwrap();

            tokens.insert(*brc20_token, token);
        }

        let _: () = context
            .rune_bridge_client(ADMIN)
            .set_bft_bridge_contract(&bft_bridge)
            .await
            .unwrap();

        context.advance_time(Duration::from_secs(2)).await;

        Self {
            bft_bridge_contract: bft_bridge,
            eth_wallet: wallet,
            inner: context,
            brc20: brc20_wallet,
            tokens,
        }
    }

    fn bridge(&self) -> Principal {
        self.inner.canisters().rune_bridge()
    }

    async fn get_deposit_address(&self, eth_address: &H160) -> String {
        self.inner
            .client(self.bridge(), ADMIN)
            .query::<_, Result<String, GetAddressError>>("get_deposit_address", (eth_address,))
            .await
            .expect("canister call failed")
            .expect("get_deposit_address error")
    }
}
