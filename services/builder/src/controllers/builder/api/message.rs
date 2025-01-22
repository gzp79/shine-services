use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum RequestMessage {
    Chat { text: String },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum ResponseMessage {
    Chat { from: Uuid, text: String },
}
