use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleOAthConfig {
    pub client_id: String,
    pub client_secret: String,
}
