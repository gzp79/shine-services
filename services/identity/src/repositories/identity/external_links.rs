use chrono::{DateTime, Utc};
use std::future::Future;
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
    fn link_user(
        &mut self,
        user_id: Uuid,
        external_user: &ExternalUserInfo,
    ) -> impl Future<Output = Result<(), IdentityError>> + Send;

    fn find_all_links(
        &mut self,
        user_id: Uuid,
    ) -> impl Future<Output = Result<Vec<ExternalLink>, IdentityError>> + Send;

    fn is_linked(
        &mut self,
        user_id: Uuid,
    ) -> impl Future<Output = Result<bool, IdentityError>> + Send;

    fn find_by_external_link(
        &mut self,
        provider: &str,
        provider_id: &str,
    ) -> impl Future<Output = Result<Option<Identity>, IdentityError>> + Send;

    fn delete_link(
        &mut self,
        user_id: Uuid,
        provider: &str,
        provider_id: &str,
    ) -> impl Future<Output = Result<Option<()>, IdentityError>> + Send;
}
