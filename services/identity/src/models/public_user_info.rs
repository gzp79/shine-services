use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicUserInfo {
    pub name: String,
}

impl PublicUserInfo {
    pub fn anonymous() -> Self {
        Self { name: "Anonymous".to_string() }
    }
}
