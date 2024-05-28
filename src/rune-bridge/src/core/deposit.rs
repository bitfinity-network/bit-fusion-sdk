use crate::core::index_provider::{OrdIndexProvider, RuneIndexProvider};
use crate::core::utxo_provider::{IcUtxoProvider, UtxoProvider};
use crate::core::DepositResult;
use crate::interface::DepositError;
use crate::key::{get_derivation_path_ic, BtcSignerType};
use crate::rune_info::{RuneInfo, RuneName};
use crate::state::State;
use bitcoin::hashes::Hash;
use bitcoin::{Address, Network};
use did::{H160, H256};
use eth_signer::sign_strategy::TransactionSigner;
use ic_exports::ic_cdk::api::management_canister::bitcoin::{GetUtxosResponse, Utxo};
use ic_stable_structures::CellStructure;
use minter_did::id256::Id256;
use minter_did::order::{MintOrder, SignedMintOrder};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

pub(crate) trait RuneDeposit {
    async fn deposit(
        &self,
        eth_address: &H160,
        amounts: &Option<HashMap<RuneName, u128>>,
    ) -> Result<Vec<DepositResult>, DepositError>;
}

static NONCE: AtomicU32 = AtomicU32::new(0);

struct RuneMintOrder {
    rune_name: RuneName,
    amount: u128,
    mint_order: SignedMintOrder,
    nonce: u32,
}

pub(crate) struct DefaultRuneDeposit<
    UTXO: UtxoProvider = IcUtxoProvider,
    INDEX: RuneIndexProvider = OrdIndexProvider,
> {
    state: Rc<RefCell<State>>,
    network: Network,
    signer: BtcSignerType,
    utxo_provider: UTXO,
    index_provider: INDEX,
}

impl DefaultRuneDeposit<IcUtxoProvider, OrdIndexProvider> {
    pub fn new(state: Rc<RefCell<State>>) -> Self {
        let state_ref = state.borrow();

        let network = state_ref.network();
        let ic_network = state_ref.ic_btc_network();
        let indexer_url = state_ref.indexer_url();
        let signer = state_ref.btc_signer();

        drop(state_ref);

        Self {
            state,
            network,
            signer,
            utxo_provider: IcUtxoProvider::new(ic_network),
            index_provider: OrdIndexProvider::new(indexer_url),
        }
    }
}

impl<UTXO: UtxoProvider, INDEX: RuneIndexProvider> DefaultRuneDeposit<UTXO, INDEX> {
    async fn get_transit_address(&self, eth_address: &H160) -> Address {
        self.signer
            .get_transit_address(eth_address, self.network)
            .await
    }

    fn validate_utxo_confirmations(
        &self,
        utxo_info: &GetUtxosResponse,
    ) -> Result<(), DepositError> {
        let min_confirmations = self.state.borrow().min_confirmations();
        let utxo_min_confirmations = utxo_info
            .utxos
            .iter()
            .map(|utxo| utxo_info.tip_height - utxo.height + 1)
            .min()
            .unwrap_or_default();

        if min_confirmations > utxo_min_confirmations {
            Err(DepositError::Pending {
                min_confirmations,
                current_confirmations: utxo_min_confirmations,
            })
        } else {
            log::trace!(
                "Current utxo confirmations {} satisfies minimum {}. Proceeding.",
                utxo_min_confirmations,
                min_confirmations
            );
            Ok(())
        }
    }

    async fn fill_rune_infos(
        &self,
        rune_amounts: &HashMap<RuneName, u128>,
    ) -> Option<Vec<(RuneInfo, u128)>> {
        match self.fill_rune_infos_from_state(rune_amounts) {
            Some(v) => Some(v),
            None => self.fill_rune_infos_from_indexer(rune_amounts).await,
        }
    }

    fn fill_rune_infos_from_state(
        &self,
        rune_amounts: &HashMap<RuneName, u128>,
    ) -> Option<Vec<(RuneInfo, u128)>> {
        let state = self.state.borrow();
        let runes = state.runes();
        let mut infos = vec![];
        for (rune_name, amount) in rune_amounts {
            infos.push((*runes.get(rune_name)?, *amount));
        }

        Some(infos)
    }

    async fn fill_rune_infos_from_indexer(
        &self,
        rune_amounts: &HashMap<RuneName, u128>,
    ) -> Option<Vec<(RuneInfo, u128)>> {
        let rune_list = self.index_provider.get_rune_list().await.ok()?;
        let runes: HashMap<RuneName, RuneInfo> = rune_list
            .iter()
            .map(|(rune_id, spaced_rune, decimals)| {
                (
                    spaced_rune.rune.into(),
                    RuneInfo {
                        name: spaced_rune.rune.into(),
                        decimals: *decimals,
                        block: rune_id.block,
                        tx: rune_id.tx,
                    },
                )
            })
            .collect();
        let mut infos = vec![];
        for (rune_name, amount) in rune_amounts {
            match runes.get(rune_name) {
                Some(v) => infos.push((*v, *amount)),
                None => {
                    log::error!("Ord indexer didn't return a rune information for rune {rune_name} that was present in an UTXO");
                    return None;
                }
            }
        }

        self.state.borrow_mut().update_rune_list(runes);

        Some(infos)
    }

    async fn create_mint_order(
        &self,
        eth_address: &H160,
        amount: u128,
        rune_info: RuneInfo,
        nonce: u32,
    ) -> Result<SignedMintOrder, DepositError> {
        log::trace!("preparing mint order");

        let (signer, mint_order) = {
            let state_ref = self.state.borrow();

            let sender_chain_id = state_ref.btc_chain_id();
            let sender = Id256::from_evm_address(eth_address, sender_chain_id);
            let src_token = Id256::from(rune_info.id());

            let recipient_chain_id = state_ref.erc20_chain_id();

            let mint_order = MintOrder {
                amount: amount.into(),
                sender,
                src_token,
                recipient: eth_address.clone(),
                dst_token: H160::default(),
                nonce,
                sender_chain_id,
                recipient_chain_id,
                name: rune_info.name_array(),
                symbol: rune_info.symbol_array(),
                decimals: rune_info.decimals(),
                approve_spender: Default::default(),
                approve_amount: Default::default(),
                fee_payer: H160::default(),
            };

            let signer = state_ref.signer().get().clone();

            (signer, mint_order)
        };

        let signed_mint_order = mint_order
            .encode_and_sign(&signer)
            .await
            .map_err(|err| DepositError::Sign(format!("{err:?}")))?;

        Ok(signed_mint_order)
    }

    async fn send_mint_order(&self, mint_order: &SignedMintOrder) -> Result<H256, DepositError> {
        log::trace!("Sending mint transaction");

        let signer = self.state.borrow().signer().get().clone();
        let sender = signer
            .get_address()
            .await
            .map_err(|err| DepositError::Sign(format!("{err:?}")))?;

        let (evm_info, evm_params) = {
            let state = self.state.borrow();

            let evm_info = state.get_evm_info();
            let evm_params = state
                .get_evm_params()
                .clone()
                .ok_or(DepositError::NotInitialized)?;

            (evm_info, evm_params)
        };

        let mut tx = minter_contract_utils::bft_bridge_api::mint_transaction(
            sender.0,
            evm_info.bridge_contract.0,
            evm_params.nonce.into(),
            evm_params.gas_price.into(),
            mint_order.to_vec(),
            evm_params.chain_id as _,
        );

        let signature = signer
            .sign_transaction(&(&tx).into())
            .await
            .map_err(|err| DepositError::Sign(format!("{err:?}")))?;

        tx.r = signature.r.0;
        tx.s = signature.s.0;
        tx.v = signature.v.0;
        tx.hash = tx.hash();

        let client = evm_info.link.get_json_rpc_client();
        let id = client
            .send_raw_transaction(tx)
            .await
            .map_err(|err| DepositError::Evm(format!("{err:?}")))?;

        self.state.borrow_mut().update_evm_params(|p| {
            if let Some(params) = p.as_mut() {
                params.nonce += 1;
            }
        });

        log::trace!("Mint transaction sent");

        Ok(id.into())
    }

    fn filter_out_used_utxos(&self, get_utxos_response: &mut GetUtxosResponse) {
        let (_, existing) = self.state.borrow().ledger().load_unspent_utxos();

        get_utxos_response.utxos.retain(|utxo| {
            !existing.iter().any(|v| {
                v.outpoint.txid.as_byte_array()[..] == utxo.outpoint.txid
                    && v.outpoint.vout == utxo.outpoint.vout
            })
        })
    }

    fn has_used_utxos(&self, utxos: &[Utxo]) -> bool {
        let (_, existing) = self.state.borrow().ledger().load_unspent_utxos();

        utxos.iter().any(|utxo| {
            existing.iter().any(|v| {
                v.outpoint.txid.as_byte_array()[..] == utxo.outpoint.txid
                    && v.outpoint.vout == utxo.outpoint.vout
            })
        })
    }

    pub async fn get_deposit_utxos(
        &self,
        transit_address: &Address,
    ) -> Result<Vec<Utxo>, DepositError> {
        let mut utxo_response = self.utxo_provider.get_utxos(transit_address).await?;
        self.filter_out_used_utxos(&mut utxo_response);

        if utxo_response.utxos.is_empty() {
            log::trace!("No utxos were found for address {transit_address}");
            return Err(DepositError::NothingToDeposit);
        }

        log::trace!(
            "Found {} utxos at the address {}",
            utxo_response.utxos.len(),
            transit_address
        );

        self.validate_utxo_confirmations(&utxo_response)?;

        Ok(utxo_response.utxos)
    }

    pub async fn get_mint_amounts(
        &self,
        utxos: &[Utxo],
        requested_amounts: &Option<HashMap<RuneName, u128>>,
    ) -> Result<(Vec<(RuneInfo, u128)>, Vec<Utxo>), DepositError> {
        let mut rune_amounts = HashMap::new();
        let mut used_utxos = vec![];

        for utxo in utxos {
            let tx_rune_amounts = self.index_provider.get_rune_amounts(utxo).await?;
            if !tx_rune_amounts.is_empty() {
                used_utxos.push(utxo.clone());
                for (rune_name, amount) in tx_rune_amounts {
                    *rune_amounts.entry(rune_name).or_default() += amount;
                }
            }
        }

        if rune_amounts.is_empty() {
            return Err(DepositError::NoRunesToDeposit);
        }

        if let Some(requested) = requested_amounts {
            if rune_amounts != *requested {
                return Err(DepositError::InvalidAmounts {
                    requested: requested.clone(),
                    actual: rune_amounts,
                });
            }
        }

        let Some(rune_info_amounts) = self.fill_rune_infos(&rune_amounts).await else {
            return Err(DepositError::Unavailable(
                "Ord indexer is in invalid state".to_string(),
            ));
        };

        Ok((rune_info_amounts, used_utxos))
    }

    fn store_mint_orders(
        &self,
        eth_address: &H160,
        transit_address: &Address,
        utxos: &[Utxo],
        mint_orders: &[RuneMintOrder],
    ) {
        let sender = Id256::from_evm_address(eth_address, self.state.borrow().erc20_chain_id());
        let mut state = self.state.borrow_mut();
        state
            .ledger_mut()
            .deposit(utxos, transit_address, get_derivation_path_ic(eth_address));

        for mint_order in mint_orders {
            state
                .mint_orders_mut()
                .push(sender, mint_order.nonce, mint_order.mint_order);
        }
    }

    async fn create_mint_orders(
        &self,
        rune_info_amounts: &[(RuneInfo, u128)],
        eth_address: &H160,
    ) -> Result<Vec<RuneMintOrder>, DepositError> {
        let mut mint_orders = vec![];
        for (rune_info, amount) in rune_info_amounts {
            let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
            let mint_order = self
                .create_mint_order(eth_address, *amount, *rune_info, nonce)
                .await?;

            mint_orders.push(RuneMintOrder {
                rune_name: rune_info.name,
                amount: *amount,
                mint_order,
                nonce,
            })
        }

        Ok(mint_orders)
    }
}

impl<UTXO: UtxoProvider, INDEX: RuneIndexProvider> RuneDeposit for DefaultRuneDeposit<UTXO, INDEX> {
    async fn deposit(
        &self,
        eth_address: &H160,
        amounts: &Option<HashMap<RuneName, u128>>,
    ) -> Result<Vec<DepositResult>, DepositError> {
        log::trace!("Requested deposit for eth address: {eth_address}");

        let transit_address = self.get_transit_address(eth_address).await;

        let utxos = self.get_deposit_utxos(&transit_address).await?;
        let (rune_info_amounts, used_utxos) = self.get_mint_amounts(&utxos, amounts).await?;
        let mint_orders = self
            .create_mint_orders(&rune_info_amounts, eth_address)
            .await?;

        if self.has_used_utxos(&used_utxos) {
            return Err(DepositError::NothingToDeposit);
        }

        self.store_mint_orders(eth_address, &transit_address, &used_utxos, &mint_orders);

        let mut results = vec![];
        for RuneMintOrder {
            rune_name,
            amount,
            mint_order,
            ..
        } in mint_orders
        {
            let result = match self.send_mint_order(&mint_order).await {
                Ok(tx_id) => DepositResult::MintRequested {
                    tx_id,
                    rune_name,
                    amount,
                },
                Err(_) => DepositResult::MintOrderSigned {
                    mint_order: Box::new(mint_order),
                    rune_name,
                    amount,
                },
            };

            results.push(result);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::WithdrawError;
    use crate::state::RuneBridgeConfig;
    use bitcoin::{FeeRate, Network, PrivateKey, Transaction};
    use ic_exports::ic_cdk::api::management_canister::bitcoin::Outpoint;
    use ord_rs::wallet::LocalSigner;
    use ordinals::{RuneId, SpacedRune};
    use std::collections::HashSet;
    use std::str::FromStr;

    struct MockUtxoProvider {
        utxos_result: Result<GetUtxosResponse, DepositError>,
    }

    impl MockUtxoProvider {
        fn set_utxo_result(&mut self, utxo_count: usize) -> Vec<Utxo> {
            let curr_height = 123;

            let mut utxos = vec![];
            for i in 0..utxo_count {
                utxos.push(Utxo {
                    outpoint: Outpoint {
                        txid: vec![i as u8 + 1; 32],
                        vout: 0,
                    },
                    value: 10_000,
                    height: curr_height - MIN_CONFIRMATIONS - i as u32,
                })
            }

            self.utxos_result = Ok(GetUtxosResponse {
                utxos: utxos.clone(),
                tip_block_hash: vec![],
                tip_height: curr_height,
                next_page: None,
            });

            utxos
        }
    }

    impl Default for MockUtxoProvider {
        fn default() -> Self {
            Self {
                utxos_result: Err(DepositError::NotScheduled),
            }
        }
    }

    impl UtxoProvider for MockUtxoProvider {
        async fn get_utxos(&self, _address: &Address) -> Result<GetUtxosResponse, DepositError> {
            self.utxos_result.clone()
        }

        async fn get_fee_rate(&self) -> Result<FeeRate, WithdrawError> {
            todo!()
        }

        async fn send_tx(&self, _transaction: &Transaction) -> Result<(), WithdrawError> {
            todo!()
        }
    }

    struct MockIndexProvider {
        rune_amounts: HashMap<Utxo, HashMap<RuneName, u128>>,
        run_before_amounts: Box<dyn Fn()>,
    }

    impl MockIndexProvider {
        fn set_rune_amount(&mut self, utxo: &Utxo, rune_name: &str, amount: u128) {
            self.rune_amounts.insert(
                utxo.clone(),
                [(RuneName::from_str(rune_name).unwrap(), amount)]
                    .into_iter()
                    .collect(),
            );
        }
    }

    impl Default for MockIndexProvider {
        fn default() -> Self {
            Self {
                rune_amounts: HashMap::new(),
                run_before_amounts: Box::new(|| ()),
            }
        }
    }

    impl RuneIndexProvider for MockIndexProvider {
        async fn get_rune_amounts(
            &self,
            utxo: &Utxo,
        ) -> Result<HashMap<RuneName, u128>, DepositError> {
            (self.run_before_amounts)();

            Ok(self.rune_amounts.get(utxo).cloned().unwrap_or_default())
        }

        async fn get_rune_list(&self) -> Result<Vec<(RuneId, SpacedRune, u8)>, DepositError> {
            let mut runes = HashSet::new();
            for amounts in self.rune_amounts.values() {
                for rune_name in amounts.keys() {
                    runes.insert(rune_name);
                }
            }

            Ok(runes
                .iter()
                .map(|rune_name| {
                    (
                        RuneId::new(123, 1).unwrap(),
                        SpacedRune::new(rune_name.inner(), Default::default()),
                        0,
                    )
                })
                .collect())
        }
    }

    const MIN_CONFIRMATIONS: u32 = 12;

    fn test_deposit() -> DefaultRuneDeposit<MockUtxoProvider, MockIndexProvider> {
        let state = State {
            config: RuneBridgeConfig {
                min_confirmations: MIN_CONFIRMATIONS,
                ..Default::default()
            },
            ..Default::default()
        };

        DefaultRuneDeposit {
            state: Rc::new(RefCell::new(state)),
            network: Network::Bitcoin,
            signer: BtcSignerType::Local(LocalSigner::new(
                PrivateKey::from_wif("5HpHagT65VRRsu5GMDiAUJYKKxg4tfjUp5SnASBcErBGxCXyxPV")
                    .unwrap(),
            )),
            utxo_provider: MockUtxoProvider::default(),
            index_provider: MockIndexProvider::default(),
        }
    }

    fn eth_address(seed: u8) -> H160 {
        [seed; H160::BYTE_SIZE].into()
    }

    fn rune_amounts(amounts: &[(&str, u128)]) -> Option<HashMap<RuneName, u128>> {
        Some(
            amounts
                .iter()
                .map(|(name, amount)| (RuneName::from_str(name).unwrap(), *amount))
                .collect(),
        )
    }

    #[tokio::test]
    async fn deposit_creates_mint_order() {
        let mut deposit = test_deposit();
        let dst = eth_address(1);

        let utxos = deposit.utxo_provider.set_utxo_result(1);
        deposit
            .index_provider
            .set_rune_amount(&utxos[0], "TEST", 10000);

        let result = deposit.deposit(&dst, &None).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], DepositResult::MintOrderSigned { .. }));

        let sender = Id256::from_evm_address(&dst, deposit.state.borrow().erc20_chain_id());
        let orders = deposit.state.borrow().orders_store.get(sender);
        assert_eq!(orders.len(), 1);

        if let DepositResult::MintOrderSigned { mint_order, .. } = &result[0] {
            assert_eq!(orders[0].1, **mint_order);
        }
    }

    #[tokio::test]
    async fn concurrent_requests_to_deposit_dont_double_spend_utxos() {
        let mut deposit = test_deposit();
        let dst = eth_address(1);
        let address = deposit.get_transit_address(&dst).await;
        let derivation_path = get_derivation_path_ic(&dst);

        let utxos = deposit.utxo_provider.set_utxo_result(1);
        deposit
            .index_provider
            .set_rune_amount(&utxos[0], "TEST", 10000);

        let state = deposit.state.clone();
        deposit.index_provider.run_before_amounts = Box::new(move || {
            state
                .borrow_mut()
                .ledger
                .deposit(&utxos, &address, derivation_path.clone());
        });

        let result = deposit.deposit(&dst, &None).await;
        assert!(
            matches!(result, Err(DepositError::NothingToDeposit)),
            "unexpected deposit result: {result:?}"
        )
    }

    #[tokio::test]
    async fn deposit_only_consumes_utxos_with_runes() {
        let mut deposit = test_deposit();
        let dst = eth_address(4);

        let utxos = deposit.utxo_provider.set_utxo_result(4);

        deposit
            .index_provider
            .set_rune_amount(&utxos[0], "TEST", 10000);
        deposit
            .index_provider
            .set_rune_amount(&utxos[2], "TESTTEST", 1000);

        let result = deposit.deposit(&dst, &None).await.unwrap();
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], DepositResult::MintOrderSigned { .. }));
        assert!(matches!(result[1], DepositResult::MintOrderSigned { .. }));

        assert!(!deposit.has_used_utxos(&[utxos[1].clone(), utxos[3].clone()]));
        assert!(deposit.has_used_utxos(&[utxos[0].clone()]));
        assert!(deposit.has_used_utxos(&[utxos[2].clone()]));
    }

    #[tokio::test]
    async fn deposit_checks_requested_amounts() {
        let mut deposit = test_deposit();
        let dst = eth_address(4);

        let utxos = deposit.utxo_provider.set_utxo_result(2);

        deposit
            .index_provider
            .set_rune_amount(&utxos[0], "TEST", 10000);
        deposit
            .index_provider
            .set_rune_amount(&utxos[1], "TESTTEST", 1000);

        assert!(matches!(
            deposit
                .deposit(&dst, &rune_amounts(&[("TEST", 10000)]))
                .await,
            Err(DepositError::InvalidAmounts { .. })
        ));
        assert!(matches!(
            deposit
                .deposit(&dst, &rune_amounts(&[("TEST", 10001), ("TESTTEST", 1000)]))
                .await,
            Err(DepositError::InvalidAmounts { .. })
        ));
        assert!(matches!(
            deposit.deposit(&dst, &rune_amounts(&[])).await,
            Err(DepositError::InvalidAmounts { .. })
        ));
        assert!(matches!(
            deposit
                .deposit(&dst, &rune_amounts(&[("TEST", 10000), ("TESTTEST", 1000)]))
                .await,
            Ok(..)
        ));
    }
}
