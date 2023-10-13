use crate::{
    db::{Permission, PermissionSet},
    services::IdentityServiceState,
};
use shine_service::{axum::Problem, service::CurrentUser};

impl IdentityServiceState {
    pub(in crate::services) async fn get_permissions(
        &self,
        current_user: &CurrentUser,
    ) -> Result<PermissionSet, Problem> {
        // all sources of permission shall be added here
        Ok(PermissionSet::from_roles(&current_user.roles))
    }

    pub(in crate::services) async fn require_permission(
        &self,
        current_user: &CurrentUser,
        permission: Permission,
    ) -> Result<(), Problem> {
        self.get_permissions(current_user)
            .await?
            .require(permission)
            .map_err(Problem::from)?;
        Ok(())
    }
}
