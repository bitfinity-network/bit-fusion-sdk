use std::borrow::Cow;

use candid::{CandidType, Decode, Deserialize, Encode};
use did::H160;
use ic_exports::ic_kit::ic;
use ic_stable_structures::{Bound, Storable};

/// Structure that contains full information about the process of an operation execution. This
/// log will contain every step of an operation execution, whether successfully executed or if it
/// resulted in an error.
///
/// The structure itself guarantees that at least one step in the log will be successful (e.g.
/// the first step - creation of the operation).
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct OperationLog<P>
where
    P: CandidType,
{
    log: Vec<OperationLogEntry<P>>,
    wallet_address: H160,
}

/// The result of a single step taken in the process of an operation execution.
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct OperationLogEntry<P>
where
    P: CandidType,
{
    /// IC timestamp when the step completed. If the operation was executed asynchronously, this
    /// field will contain the timestamp of the last IC executor run.
    pub time_stamp: u64,
    /// Result of the execution step. If `Ok`, will contain the updated state of the operation. If
    /// `Err` - error message. In case of an error, the state of the operation is guaranteed to
    /// have not been changed.
    pub step_result: Result<P, String>,
}

impl<P> OperationLog<P>
where
    P: CandidType,
{
    /// Creates a new operation log with a single entry - creation of the operation with the given
    /// payload. `wallet_address` parameter is the address of the ETH wallet that initiated the
    /// operation.
    pub fn new(payload: P, wallet_address: H160) -> Self {
        Self {
            log: vec![OperationLogEntry {
                time_stamp: Self::timestamp(),
                step_result: Ok(payload),
            }],
            wallet_address,
        }
    }

    /// Operation state of the last successful step in the log.
    pub fn current_step(&self) -> &P {
        // Since the log structure guarantees that there will be at least one successful step,
        // we can `expect` it here.
        self.log
            .iter()
            .rev()
            .filter_map(|entry| entry.step_result.as_ref().ok())
            .next()
            .expect("operation log does not contain a successful step")
    }

    /// Adds a new entry to the log with the given result.
    pub fn add_step(&mut self, step_result: Result<P, String>) {
        self.log.push(OperationLogEntry {
            time_stamp: Self::timestamp(),
            step_result,
        });
    }

    /// Address of the ETH wallet that initiated this operation.
    pub fn wallet_address(&self) -> &H160 {
        &self.wallet_address
    }

    /// Returns log entries of the operation log.
    pub fn log(&self) -> &Vec<OperationLogEntry<P>> {
        &self.log
    }

    fn timestamp() -> u64 {
        ic::time()
    }
}

impl<P> Storable for OperationLog<P>
where
    P: CandidType + Clone + for<'de> Deserialize<'de>,
{
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).expect("failed to encode operation log entry"))
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to decode operation log entry")
    }

    const BOUND: Bound = Bound::Unbounded;
}
