use std::collections::HashMap;
use std::rc::Rc;

use bridge_did::error::{BftResult, Error};
use bridge_did::op_id::OperationId;

pub mod mint_tx;
pub mod sing_orders;

// The async-trait macro is necessary to make the trait object safe.
#[async_trait::async_trait(?Send)]
pub trait BridgeService {
    async fn run(&self) -> BftResult<()>;

    fn push_operation(&self, id: OperationId) -> BftResult<()>;
}

pub type ServiceId = u64;

/// Describes when service should run.
pub enum ServiceOrder {
    BeforeOperations,
    AfterOperations,
}

pub type DynService = Rc<dyn BridgeService>;

/// Services in the runtime.
#[derive(Default)]
pub struct Services {
    before: HashMap<ServiceId, DynService>,
    after: HashMap<ServiceId, DynService>,
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
    pub fn push_operation(&self, service_id: ServiceId, op_id: OperationId) -> BftResult<()> {
        let Some(service) = self
            .before
            .get(&service_id)
            .or_else(|| self.after.get(&service_id))
        else {
            log::warn!("Failed to push task to start service #{service_id}. Service not found.");
            return Err(Error::ServiceNotFound);
        };

        service.push_operation(op_id)?;
        Ok(())
    }

    /// Services getter.
    pub fn services(&self, order: ServiceOrder) -> &HashMap<ServiceId, DynService> {
        match order {
            ServiceOrder::BeforeOperations => &self.before,
            ServiceOrder::AfterOperations => &self.after,
        }
    }

    fn mut_services(&mut self, order: ServiceOrder) -> &mut HashMap<ServiceId, DynService> {
        match order {
            ServiceOrder::BeforeOperations => &mut self.before,
            ServiceOrder::AfterOperations => &mut self.after,
        }
    }
}
