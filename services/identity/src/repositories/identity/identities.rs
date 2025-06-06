use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use utoipa::ToSchema;
use uuid::Uuid;

use super::IdentityError;

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
    pub email: Option<String>,
    pub is_email_confirmed: bool,
    pub created: DateTime<Utc>,
}

/// Handle identities.
pub trait Identities {
    /// Try to create a new user.
    /// @param user_id: The user id.
    /// @param user_name: The user name.
    /// @param email: The email address and whether it is confirmed.
    fn create_user(
        &mut self,
        user_id: Uuid,
        user_name: &str,
        email: Option<(&str, bool)>,
    ) -> impl Future<Output = Result<Identity, IdentityError>> + Send;

    fn find_by_id(&mut self, id: Uuid) -> impl Future<Output = Result<Option<Identity>, IdentityError>> + Send;
    fn find_by_email(&mut self, id: &str) -> impl Future<Output = Result<Option<Identity>, IdentityError>> + Send;

    fn update(
        &mut self,
        id: Uuid,
        name: Option<&str>,
        email: Option<(&str, bool)>,
    ) -> impl Future<Output = Result<Option<Identity>, IdentityError>> + Send;

    fn cascaded_delete(&mut self, id: Uuid) -> impl Future<Output = Result<(), IdentityError>> + Send;
}
