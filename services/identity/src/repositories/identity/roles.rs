use std::future::Future;
use uuid::Uuid;

use super::IdentityError;

/// Handle user roles.
pub trait Roles {
    fn add_role(
        &mut self,
        user_id: Uuid,
        role: &str,
    ) -> impl Future<Output = Result<Option<Vec<String>>, IdentityError>> + Send;

    fn get_roles(
        &mut self,
        user_id: Uuid,
    ) -> impl Future<Output = Result<Option<Vec<String>>, IdentityError>> + Send;

    fn delete_role(
        &mut self,
        user_id: Uuid,
        role: &str,
    ) -> impl Future<Output = Result<Option<Vec<String>>, IdentityError>> + Send;
}
