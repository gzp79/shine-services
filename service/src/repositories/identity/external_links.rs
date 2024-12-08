use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{Identity, IdentityError};

#[derive(Clone, Debug)]
pub struct ExternalUserInfo {
    pub provider: String,
    pub provider_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ExternalLink {
    pub user_id: Uuid,
    pub provider: String,
    pub provider_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub linked_at: DateTime<Utc>,
}

/// Handle external links
pub trait ExternalLinks {
    async fn link_user(&mut self, user_id: Uuid, external_user: &ExternalUserInfo) -> Result<(), IdentityError>;

    async fn find_all_links(&mut self, user_id: Uuid) -> Result<Vec<ExternalLink>, IdentityError>;

    async fn is_linked(&mut self, user_id: Uuid) -> Result<bool, IdentityError>;

    async fn find_by_external_link(
        &mut self,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<Identity>, IdentityError>;

    async fn delete_link(
        &mut self,
        user_id: Uuid,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<()>, IdentityError>;
}
