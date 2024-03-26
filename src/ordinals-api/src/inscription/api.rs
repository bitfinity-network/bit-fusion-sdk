use super::state::*;
use crate::http::{http_get_req, PaginatedResp};

/// Retrieves a single inscription by id.
pub async fn get_inscription_by_id(
    base_api_url: &str,
    id: &str,
) -> Result<Option<Inscription>, String> {
    http_get_req::<Inscription>(&format!("{base_api_url}/ordinals/v1/inscriptions/{id}")).await
}

/// Retrieves all transfers for a single inscription.
pub async fn get_inscription_transfers_by_id(
    base_api_url: &str,
    id: &str,
    offset: u64,
    limit: u64,
) -> Result<Option<PaginatedResp<InscriptionLocation>>, String> {
    http_get_req::<PaginatedResp<InscriptionLocation>>(&format!(
        "{base_api_url}/ordinals/v1/inscriptions/{id}/transfers?offset={offset}&limit={limit}"
    ))
    .await
}
