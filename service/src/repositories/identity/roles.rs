use uuid::Uuid;

use super::IdentityError;

pub type Role = String;

/// Handle user roles.
pub trait Roles {
    async fn add_role(&mut self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError>;
    async fn get_roles(&mut self, user_id: Uuid) -> Result<Option<Vec<String>>, IdentityError>;
    async fn delete_role(&mut self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError>;
}
