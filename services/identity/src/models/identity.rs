use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_infra::models::Email;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum IdentityKind {
    User,
    Studio,
}

#[derive(Clone, Debug)]
pub struct Identity {
    pub id: Uuid,
    pub kind: IdentityKind,
    pub name: String,
    pub email: Option<Email>,
    pub is_email_confirmed: bool,
    pub created: DateTime<Utc>,
}
