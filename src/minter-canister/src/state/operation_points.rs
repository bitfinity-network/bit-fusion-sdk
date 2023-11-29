use std::cell::RefCell;

use candid::Principal;
use did::ic::StorablePrincipal;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{
    BTreeMapStructure, CellStructure, StableBTreeMap, StableCell, VirtualMemory,
};
use minter_did::error::{Error, Result};
use minter_did::init::OperationPricing;

use crate::constant::{OPERATION_PRICING_MEMORY_ID, USER_OPERATION_POINTS_MEMORY_ID};
use crate::memory::MEMORY_MANAGER;

#[derive(Clone)]
pub struct OperationPoints {
    pub owner: Principal,
}

impl OperationPoints {
    pub fn new(owner: Principal) -> Self {
        Self { owner }
    }

    /// Update owner principal
    pub fn set_owner(&mut self, owner: Principal) {
        self.owner = owner;
    }

    /// Returns pricing.
    pub fn get_pricing(&self) -> OperationPricing {
        PRICING_CELL.with(|cell| *cell.borrow().get())
    }

    /// Returns pricing.
    pub fn set_pricing(&mut self, pricing: OperationPricing) {
        PRICING_CELL.with(|cell| {
            cell.borrow_mut()
                .set(pricing)
                .expect("failed to set pricing")
        });
    }

    /// Return user's points.
    pub fn get_user_points(&self, user: Principal) -> u32 {
        if user == self.owner {
            return u32::MAX;
        }
        let key = StorablePrincipal(user);

        USER_POINTS
            .with(|map| map.borrow().get(&key))
            .unwrap_or_default()
    }

    /// Increases user's points by `pricing.evmc_notification`.
    pub fn add_evmc_tx_points(&mut self, user: Principal) -> u32 {
        let value = self.with_princing(|pricing| pricing.evmc_notification);
        self.add_points(user, value)
    }

    /// Decreases user's points by `pricing.evm_registration`.
    /// Returns Err(Error::InsufficientOperationPoints) on failure.
    pub fn deduct_evm_registration_fee(&mut self, user: Principal) -> Result<u32> {
        let fee = self.with_princing(|pricing| pricing.evm_registration);
        self.deduct_points(user, fee)
    }

    /// Decreases user's points by `pricing.icrc_mint`.
    /// Returns Err(Error::InsufficientOperationPoints) on failure.
    pub fn deduct_icrc_mint_fee(&mut self, user: Principal) -> Result<u32> {
        let fee = self.with_princing(|pricing| pricing.icrc_mint_approval);
        self.deduct_points(user, fee)
    }

    /// Decreases user's points by `pricing.icrc_transfer`.
    /// Returns Err(Error::InsufficientOperationPoints) on failure.
    pub fn deduct_icrc_transfer_fee(&mut self, user: Principal) -> Result<u32> {
        let fee = self.with_princing(|pricing| pricing.icrc_transfer);
        self.deduct_points(user, fee)
    }

    /// Decreases user's points by `pricing.erc20_mint`.
    /// Returns Err(Error::InsufficientOperationPoints) on failure.
    pub fn deduct_erc20_mint_fee(&mut self, user: Principal) -> Result<u32> {
        let fee = self.with_princing(|pricing| pricing.erc20_mint);
        self.deduct_points(user, fee)
    }

    /// Decreases user's points by `pricing.endpoint_query`.
    /// Returns Err(Error::InsufficientOperationPoints) on failure.
    pub fn deduct_endpoint_query_fee(&mut self, user: Principal) -> Result<u32> {
        let fee = self.with_princing(|pricing| pricing.endpoint_query);
        self.deduct_points(user, fee)
    }

    /// Returns Ok(new balance) on success.
    /// Returns Err(Error::InsufficientOperationPoints) on failure.
    fn deduct_points(&mut self, user: Principal, value: u32) -> Result<u32> {
        // check exmpted users
        if user == self.owner {
            return Ok(u32::MAX);
        }

        USER_POINTS.with(|map| {
            let mut map = map.borrow_mut();
            let key = StorablePrincipal(user);
            let old_balance = map.get(&key).unwrap_or_default();

            let new_balance =
                old_balance
                    .checked_sub(value)
                    .ok_or(Error::InsufficientOperationPoints {
                        expected: value,
                        got: old_balance,
                    })?;

            match new_balance {
                0 => map.remove(&key),
                non_zero => map.insert(key, non_zero),
            };

            Ok(new_balance)
        })
    }

    /// Increases the balance of the given user for the given value.
    /// Saturates u32 on overflow.
    fn add_points(&mut self, user: Principal, value: u32) -> u32 {
        if user == self.owner {
            return u32::MAX;
        }

        USER_POINTS.with(
            |map: &RefCell<
                StableBTreeMap<StorablePrincipal, u32, VirtualMemory<DefaultMemoryImpl>>,
            >| {
                let mut map = map.borrow_mut();
                let key = StorablePrincipal(user);

                let old_balance = map.get(&key).unwrap_or_default();
                let new_balance = old_balance.saturating_add(value);
                map.insert(key, new_balance);
                new_balance
            },
        )
    }

    fn with_princing<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&OperationPricing) -> R,
    {
        PRICING_CELL.with(|cell| f(cell.borrow().get()))
    }

    pub fn clear(&self) {
        USER_POINTS.with(|map| map.borrow_mut().clear());
        PRICING_CELL
            .with(|cell| cell.borrow_mut().set(OperationPricing::default()))
            .expect("failed to clear pricing stable cell");
    }
}

thread_local! {
    static PRICING_CELL: RefCell<StableCell<OperationPricing, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(OPERATION_PRICING_MEMORY_ID)), OperationPricing::default())
        .expect("failed to initialize pricing cell"));

    static USER_POINTS: RefCell<StableBTreeMap<StorablePrincipal, u32, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(USER_OPERATION_POINTS_MEMORY_ID))));
}

#[cfg(test)]
mod tests {
    use candid::Principal;
    use ic_exports::ic_kit::MockContext;
    use ic_stable_structures::{BTreeMapStructure, Storable};
    use minter_did::error::Error;

    use super::{OperationPoints, OperationPricing, StorablePrincipal};
    use crate::state::operation_points::USER_POINTS;

    #[test]
    fn principal_key_serialization() {
        let principal = StorablePrincipal(Principal::anonymous());
        let encoded = principal.to_bytes();
        let decoded = StorablePrincipal::from_bytes(encoded);
        assert_eq!(principal, decoded);
    }

    #[test]
    fn test_pricing_storage() {
        MockContext::new().inject();
        let mut points = OperationPoints::new(Principal::anonymous());
        assert_eq!(points.get_pricing(), OperationPricing::default());

        let new_pricing = OperationPricing {
            evmc_notification: 1000,
            evm_registration: 5,
            icrc_mint_approval: 20,
            icrc_transfer: 25,
            erc20_mint: 30,
            endpoint_query: 1,
        };
        points.set_pricing(new_pricing);
        assert_eq!(points.get_pricing(), new_pricing)
    }

    #[test]
    fn test_points_update() {
        MockContext::new().inject();
        let mut points = OperationPoints::new(Principal::anonymous());
        let new_pricing = OperationPricing {
            evmc_notification: 300,
            evm_registration: 40,
            icrc_mint_approval: 50,
            icrc_transfer: 60,
            erc20_mint: 70,
            endpoint_query: 80,
        };
        points.set_pricing(new_pricing);

        let user = Principal::management_canister();

        points.add_evmc_tx_points(user);
        let mut expected_balance = new_pricing.evmc_notification;
        assert_eq!(points.get_user_points(user), expected_balance);

        points.deduct_evm_registration_fee(user).unwrap();
        expected_balance -= new_pricing.evm_registration;
        assert_eq!(points.get_user_points(user), expected_balance);

        points.deduct_icrc_mint_fee(user).unwrap();
        expected_balance -= new_pricing.icrc_mint_approval;
        assert_eq!(points.get_user_points(user), expected_balance);

        points.deduct_icrc_transfer_fee(user).unwrap();
        expected_balance -= new_pricing.icrc_transfer;
        assert_eq!(points.get_user_points(user), expected_balance);

        points.deduct_erc20_mint_fee(user).unwrap();
        expected_balance -= new_pricing.erc20_mint;
        assert_eq!(points.get_user_points(user), expected_balance);

        points.deduct_endpoint_query_fee(user).unwrap();
        expected_balance -= new_pricing.endpoint_query;
        assert_eq!(points.get_user_points(user), expected_balance);

        assert_eq!(expected_balance, 0);

        assert!(USER_POINTS.with(|map| map.borrow().is_empty()));

        let err = points.deduct_erc20_mint_fee(user).unwrap_err();
        assert_eq!(
            err,
            Error::InsufficientOperationPoints {
                expected: new_pricing.erc20_mint,
                got: 0
            }
        )
    }

    #[test]
    fn should_exempt_users() {
        let user = Principal::management_canister();

        MockContext::new().inject();
        let mut points = OperationPoints::new(Principal::management_canister());
        let new_pricing = OperationPricing {
            evmc_notification: 300,
            evm_registration: 40,
            icrc_mint_approval: 50,
            icrc_transfer: 60,
            erc20_mint: 70,
            endpoint_query: 80,
        };
        points.set_pricing(new_pricing);

        assert_eq!(points.get_user_points(user), u32::MAX);
        assert_eq!(points.deduct_endpoint_query_fee(user), Ok(u32::MAX));

        let not_exempted = Principal::anonymous();
        assert_eq!(points.get_user_points(not_exempted), 0);
        assert!(points.deduct_endpoint_query_fee(not_exempted).is_err());
    }

    #[test]
    fn should_update_owner() {
        let mut points = OperationPoints::new(Principal::management_canister());
        points.set_owner(Principal::anonymous());
        assert_eq!(points.owner, Principal::anonymous());
    }
}
