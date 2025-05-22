use shine_infra::web::session::{roles, CorePermissions, CurrentUser, PermissionSet};

pub mod permissions {
    pub use shine_infra::web::session::permissions::*;

    /// Allow to query the general information of an identity
    pub const READ_ANY_IDENTITY: &str = "ReadAnyIdentity";
    /// Allow to get the roles of any user
    pub const READ_ANY_USER_ROLE: &str = "ReadAnyUserRole";
    /// Allow to update the roles of any user
    pub const UPDATE_ANY_USER_ROLE: &str = "UpdateAnyUserRole";
}

pub trait IdentityPermissions {
    fn identity_permissions(&self) -> PermissionSet;
}

impl IdentityPermissions for CurrentUser {
    fn identity_permissions(&self) -> PermissionSet {
        let mut permission = self.core_permissions();

        for role in &self.roles {
            match role.as_str() {
                roles::SUPER_ADMIN => {
                    permission.add(permissions::READ_ANY_IDENTITY);
                    permission.add(permissions::READ_ANY_USER_ROLE);
                    permission.add(permissions::UPDATE_ANY_USER_ROLE);
                }
                roles::USER_ADMIN => {
                    permission.add(permissions::READ_ANY_IDENTITY);
                    permission.add(permissions::READ_ANY_USER_ROLE);
                }
                _ => {}
            };
        }

        permission
    }
}
