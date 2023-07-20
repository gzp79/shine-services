use crate::{db::PermissionSet, services::IdentityServiceState};
use shine_service::service::CurrentUser;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
#[error("Failed to get permission")]
pub(in crate::services) struct GetPermissionError;

impl IdentityServiceState {
    pub(in crate::services) async fn get_permissions(
        &self,
        current_user: &CurrentUser,
    ) -> Result<PermissionSet, GetPermissionError> {
        // all sources of permission shall be added here
        Ok(PermissionSet::from_roles(&current_user.roles))
    }
}
