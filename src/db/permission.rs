use std::collections::HashSet;
use thiserror::Error as ThisError;

/// All the user roles used by the service.
pub mod roles {
    pub const SUPER_ADMIN: &str = "SuperAdmin";
    pub const USER_ADMIN: &str = "UserAdmin";
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Permission {
    GetUserRole,
    UpdateUserRole,
}

#[derive(Debug, ThisError)]
pub enum PermissionError {
    #[error("Missing {0:?} permission to perform the operation")]
    MissingPermission(Permission),
}

pub struct PermissionSet {
    permission: HashSet<Permission>,
}

impl PermissionSet {
    pub fn from_roles(roles: &[String]) -> Self {
        let mut permission = HashSet::new();
        for role in roles {
            match role.as_str() {
                roles::SUPER_ADMIN => {
                    permission.insert(Permission::GetUserRole);
                    permission.insert(Permission::UpdateUserRole);
                }
                roles::USER_ADMIN => {
                    permission.insert(Permission::GetUserRole);
                    permission.insert(Permission::UpdateUserRole);
                }
                _ => {}
            };
        }

        Self { permission }
    }

    pub fn require(&self, permission: Permission) -> Result<(), PermissionError> {
        if self.permission.contains(&permission) {
            Ok(())
        } else {
            Err(PermissionError::MissingPermission(permission))
        }
    }
}
