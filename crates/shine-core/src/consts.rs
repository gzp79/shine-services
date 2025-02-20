use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    En,
    Hu,
}
