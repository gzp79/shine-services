use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ExternalLink {
    pub user_id: Uuid,
    pub provider: String,
    pub provider_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub linked_at: DateTime<Utc>,
}
