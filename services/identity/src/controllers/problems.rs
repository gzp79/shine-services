use crate::services::{PermissionError, SessionUserSyncError};
use shine_core::axum::{IntoProblem, Problem, ProblemConfig};

impl IntoProblem for PermissionError {
    fn into_problem(self, _config: &ProblemConfig) -> Problem {
        match self {
            PermissionError::MissingPermission(perm) => {
                Problem::forbidden().with_detail(format!("Missing [{:?}] permission", perm))
            }
        }
    }
}

impl IntoProblem for SessionUserSyncError {
    fn into_problem(self, config: &ProblemConfig) -> Problem {
        match self {
            SessionUserSyncError::UserNotFound(user_id) => {
                Problem::not_found().with_instance_str(format!("{{identity_api}}/identities/{}", user_id))
            }
            SessionUserSyncError::RolesNotFound(user_id) => {
                Problem::not_found().with_instance_str(format!("{{identity_api}}/identities/{}", user_id))
            }
            err => Problem::internal_error(config, "Failed to synchronize session user", err),
        }
    }
}
