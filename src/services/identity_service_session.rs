use crate::{
    db::{Identity, Role},
    services::IdentityServiceState,
};
use shine_service::axum::Problem;
use uuid::Uuid;

impl IdentityServiceState {
    async fn get_user_info(&self, user_id: Uuid) -> Result<(Identity, Vec<Role>), Problem> {
        // get the version first as newer role is fine, but a deprecated role set is not ok
        // this order ensures the role and other data are at least as fresh as the version
        let identity = self
            .identity_manager()
            .find(crate::db::FindIdentity::UserId(user_id))
            .await
            .map_err(Problem::internal_error_from)?
            .ok_or_else(Problem::not_found)?;

        let roles = self
            .identity_manager()
            .get_roles(user_id)
            .await
            .map_err(Problem::internal_error_from)?;

        Ok((identity, roles))
    }

    pub(in crate::services) async fn update_session(&self, user_id: Uuid) -> Result<(Identity, Vec<Role>), Problem> {
        match self.get_user_info(user_id).await {
            Ok((identity, roles)) => {
                // at this point the DB has been updated, thus any new session will contain the information
                // not older than the queried user info, thus it should be not an issue if a users sign in
                // during this update process.
                self.session_manager()
                    .update_all(&identity, &roles)
                    .await
                    .map_err(Problem::internal_error_from)?;
                Ok((identity, roles))
            }
            Err(err) => {
                self.session_manager()
                    .remove_all(user_id)
                    .await
                    .map_err(Problem::internal_error_from)?;
                Err(err)
            }
        }
    }
}
