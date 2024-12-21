use shine_core::service::CurrentUser;
use std::collections::HashSet;
use thiserror::Error as ThisError;

/// All the user roles used by the service.
pub mod roles {
    pub const SUPER_ADMIN: &str = "SuperAdmin";
    pub const USER_ADMIN: &str = "UserAdmin";
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Permission {
    /// Allow to update tracing configuration
    UpdateTrace,
    /// Allow to query the general information of an identity
    ReadAnyIdentity,
    /// Allow to get the roles of any user
    ReadAnyUserRole,
    /// Allow to update the roles of any user
    UpdateAnyUserRole,
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
                    permission.insert(Permission::ReadAnyIdentity);
                    permission.insert(Permission::ReadAnyUserRole);
                    permission.insert(Permission::UpdateAnyUserRole);
                }
                roles::USER_ADMIN => {
                    permission.insert(Permission::ReadAnyUserRole);
                    permission.insert(Permission::ReadAnyIdentity);
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

impl From<&CurrentUser> for PermissionSet {
    fn from(value: &CurrentUser) -> Self {
        Self::from_roles(&value.roles)
    }
}
