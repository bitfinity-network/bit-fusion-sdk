use std::time::Duration;

use evm_canister_client::CanisterClient;
use ic_metrics::MetricsData;

/// Collect EVM canister metrics
pub async fn get_metrics(client: &impl CanisterClient) -> anyhow::Result<MetricsData> {
    let metrics: MetricsData = client.query("get_curr_metrics", ()).await?;

    Ok(metrics)
}

pub async fn admin_set_transaction_processing_interval(
    client: &impl CanisterClient,
    interval: Duration,
) -> anyhow::Result<()> {
    let _res: Option<()> = client
        .update(
            "admin_set_transaction_processing_interval",
            (interval.as_millis() as u64,),
        )
        .await?;

    Ok(())
}
