use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::identity_error::IdentityError;

#[derive(Debug, Clone, Copy)]
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
    pub version: i32,
}

/// Handle identities.
pub trait Identities {
    async fn create_user(&mut self, user_id: Uuid, user_name: &str, email: Option<&str>)
        -> Result<Identity, IdentityError>;

    async fn find_by_id(&mut self, user_id: Uuid) -> Result<Option<Identity>, IdentityError>;

    async fn cascaded_delete(&mut self, user_id: Uuid) -> Result<(), IdentityError>;
}
