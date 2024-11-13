use std::collections::HashMap;
use std::rc::Rc;

use bridge_did::error::{BTFResult, Error};
use bridge_did::op_id::OperationId;

pub mod fetch_logs;
pub mod mint_tx;
pub mod sign_orders;
pub mod timer;
pub mod update_evm_params;

// The async-trait macro is necessary to make the trait object safe.
#[async_trait::async_trait(?Send)]
pub trait BridgeService {
    async fn run(&self) -> BTFResult<()>;

    fn push_operation(&self, id: OperationId) -> BTFResult<()>;
}

pub type ServiceId = u64;

/// Describes when service should run.
pub enum ServiceOrder {
    BeforeOperations,
    ConcurrentWithOperations,
}

pub type DynService = Rc<dyn BridgeService>;

/// Services in the runtime.
#[derive(Default)]
pub struct Services {
    before: HashMap<ServiceId, DynService>,
    concurrent: HashMap<ServiceId, DynService>,
}

impl Services {
    /// Adds a service to the runtime.
    pub fn add_service(&mut self, order: ServiceOrder, id: ServiceId, service: DynService) {
        let services = self.mut_services(order);
        if services.insert(id, service).is_some() {
            panic!("Failed to add service with id#{id}. Service with this id already present.")
        };
    }

    /// Asks a start service with the given id to process operation with the given OperationId.
    pub fn push_operation(&self, service_id: ServiceId, op_id: OperationId) -> BTFResult<()> {
        let Some(service) = self
            .before
            .get(&service_id)
            .or_else(|| self.concurrent.get(&service_id))
        else {
            log::warn!("Failed to push task to service #{service_id}. Service not found.");
            return Err(Error::ServiceNotFound);
        };

        service.push_operation(op_id)?;
        Ok(())
    }

    /// Services getter.
    pub fn services(&self, order: ServiceOrder) -> &HashMap<ServiceId, DynService> {
        match order {
            ServiceOrder::BeforeOperations => &self.before,
            ServiceOrder::ConcurrentWithOperations => &self.concurrent,
        }
    }

    fn mut_services(&mut self, order: ServiceOrder) -> &mut HashMap<ServiceId, DynService> {
        match order {
            ServiceOrder::BeforeOperations => &mut self.before,
            ServiceOrder::ConcurrentWithOperations => &mut self.concurrent,
        }
    }
}
