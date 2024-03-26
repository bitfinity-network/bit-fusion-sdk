pub mod canister;

use canister::OrdinalsApiTester;
use ic_metrics::Metrics;

pub fn idl() -> String {
    let signature_verification_idl = OrdinalsApiTester::idl();
    let mut metrics_idl = <OrdinalsApiTester as Metrics>::get_idl();
    metrics_idl.merge(&signature_verification_idl);

    candid::pretty::candid::compile(&metrics_idl.env.env, &Some(metrics_idl.actor))
}
