use crate::web::{CurrentUser, IntoProblem, Problem, ProblemConfig};
use std::collections::HashSet;
use thiserror::Error as ThisError;

/// Global user roles used by the services.
pub mod roles {
    pub const SUPER_ADMIN: &str = "SuperAdmin";
    pub const USER_ADMIN: &str = "UserAdmin";
}

/// Global permissions used by the services.
pub mod permissions {
    /// Allow to update tracing configuration
    pub const UPDATE_TRACE: &str = "UpdateTrace";
}

#[derive(Debug, ThisError)]
pub enum PermissionError {
    #[error("Missing {0:?} permission to perform the operation")]
    MissingPermission(&'static str),
}

impl IntoProblem for PermissionError {
    fn into_problem(self, _config: &ProblemConfig) -> Problem {
        match self {
            PermissionError::MissingPermission(perm) => {
                Problem::forbidden().with_detail(format!("Missing [{:?}] permission", perm))
            }
        }
    }
}

#[derive(Default)]
pub struct PermissionSet {
    permission: HashSet<&'static str>,
}

impl PermissionSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, permission: &'static str) {
        self.permission.insert(permission);
    }

    pub fn remove(&mut self, permission: &'static str) {
        self.permission.remove(permission);
    }

    pub fn require(&self, permission: &'static str) -> Result<(), PermissionError> {
        if self.permission.contains(&permission) {
            Ok(())
        } else {
            Err(PermissionError::MissingPermission(permission))
        }
    }

    pub fn check(&self, permission: &'static str, config: &ProblemConfig) -> Result<(), Problem> {
        self.require(permission).map_err(|err| err.into_problem(config))
    }
}

pub trait GlobalPermissions {
    fn global_permissions(&self) -> PermissionSet;
}

impl GlobalPermissions for CurrentUser {
    fn global_permissions(&self) -> PermissionSet {
        let mut permission = PermissionSet::new();

        for role in &self.roles {
            #[allow(clippy::single_match)]
            match role.as_str() {
                roles::SUPER_ADMIN => {
                    permission.add(permissions::UPDATE_TRACE);
                }
                _ => {}
            };
        }

        permission
    }
}
