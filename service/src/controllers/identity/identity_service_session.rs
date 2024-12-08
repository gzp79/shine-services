use crate::{
    identity::IdentityServiceState,
    repositories::{Identity, Role},
};
use shine_service::axum::{Problem, ProblemConfig};
use uuid::Uuid;

impl IdentityServiceState {
    async fn get_user_info(
        &self,
        user_id: Uuid,
        problem_config: &ProblemConfig,
    ) -> Result<(Identity, Vec<Role>), Problem> {
        // get the version first as newer role is fine, but a deprecated role set is not ok
        // this order ensures the role and other data are at least as fresh as the version
        let identity = self
            .identity_manager()
            .find_by_id(user_id)
            .await
            .map_err(|err| Problem::internal_error(problem_config, "Failed to get identity", err))?
            .ok_or_else(|| {
                Problem::not_found().with_instance_str(format!("{{identity_api}}/identities/{}", user_id))
            })?;

        let roles = self
            .identity_manager()
            .get_roles(user_id)
            .await
            .map_err(|err| Problem::internal_error(problem_config, "Failed to get roles", err))?
            .ok_or_else(|| {
                Problem::not_found().with_instance_str(format!("{{identity_api}}/identities/{}", user_id))
            })?;

        Ok((identity, roles))
    }

    pub(in crate::identity) async fn update_session(
        &self,
        user_id: Uuid,
        problem_config: &ProblemConfig,
    ) -> Result<(Identity, Vec<Role>), Problem> {
        match self.get_user_info(user_id, problem_config).await {
            Ok((identity, roles)) => {
                // at this point the DB has been updated, thus any new session will contain the information
                // not older than the queried user info, thus it should be not an issue if a users sign in
                // during this update process.
                self.session_manager()
                    .update_all(&identity, &roles)
                    .await
                    .map_err(|err| Problem::internal_error(problem_config, "Failed to update session", err))?;
                Ok((identity, roles))
            }
            Err(err) => {
                self.session_manager()
                    .remove_all(user_id)
                    .await
                    .map_err(|err| Problem::internal_error(problem_config, "Failed to revoke sessions", err))?;
                Err(err)
            }
        }
    }
}
