use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurgeGuestsResult {
    pub deleted: usize,
    pub has_more: bool,
}
