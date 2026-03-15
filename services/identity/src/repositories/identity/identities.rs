use crate::models::{Identity, IdentityError};
use chrono::{DateTime, Utc};
use std::future::Future;
use uuid::Uuid;

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

    /// Delete guest users (no confirmed email, no external links) created before `cutoff`.
    /// Returns the UUIDs of deleted users. At most `limit` users are deleted per call.
    fn delete_guests(
        &mut self,
        cutoff: DateTime<Utc>,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<Uuid>, IdentityError>> + Send;
}
