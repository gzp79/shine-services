use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use uuid::Uuid;

use super::IdentityError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
    pub version: i32,
}

/// Handle identities.
pub trait Identities {
    fn create_user(
        &mut self,
        user_id: Uuid,
        user_name: &str,
        email: Option<&str>,
    ) -> impl Future<Output = Result<Identity, IdentityError>> + Send;

    fn find_by_id(&mut self, user_id: Uuid) -> impl Future<Output = Result<Option<Identity>, IdentityError>> + Send;

    fn cascaded_delete(&mut self, user_id: Uuid) -> impl Future<Output = Result<(), IdentityError>> + Send;
}
