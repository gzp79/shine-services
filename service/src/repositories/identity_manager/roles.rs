use crate::repositories::{IdentityBuildError, IdentityError};
use postgres_from_row::FromRow;
use shine_service::{
    pg_query,
    service::{PGClient, PGConnection, PGErrorChecks as _, PGRawConnection},
};
use uuid::Uuid;

use super::{versioned_update::VersionedUpdate, versioned_update::VersionedUpdateStatements};

pub type Role = String;

pg_query!( AddUserRole =>
    in = user_id: Uuid, role: &str;
    sql = r#"
        INSERT INTO roles (user_id, role) 
            VALUES ($1, $2)
    "#
);

#[derive(FromRow)]
struct UserRolesRow {
    roles: Vec<String>,
}

pg_query!( GetUserRoles =>
    in = user_id: Uuid;
    out = UserRolesRow;
    sql = r#"
        SELECT 
            CASE 
                WHEN array_agg(r.role) = ARRAY[NULL]::text[] THEN ARRAY[]::text[] 
                ELSE array_agg(r.role) 
            END AS roles
        FROM identities i
        LEFT JOIN roles r ON i.user_id = r.user_id
        WHERE i.user_id = $1
        GROUP BY i.user_id
    "#
);

pg_query!( DeleteUserRole =>
    in = user_id: Uuid, role: &str;
    sql = r#"
        DELETE FROM roles WHERE user_id = $1 AND role = $2
    "#
);

pub struct RolesStatements {
    add: AddUserRole,
    get: GetUserRoles,
    delete: DeleteUserRole,
}

impl RolesStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            add: AddUserRole::new(client).await?,
            get: GetUserRoles::new(client).await?,
            delete: DeleteUserRole::new(client).await?,
        })
    }
}

/// Roles Data Access Object.
pub struct Roles<'a, T>
where
    T: PGRawConnection,
{
    client: &'a mut PGConnection<T>,
    stmts_version: &'a VersionedUpdateStatements,
    stmts_role: &'a RolesStatements,
}

impl<'a, T> Roles<'a, T>
where
    T: PGRawConnection,
{
    pub fn new(
        client: &'a mut PGConnection<T>,
        stmts_version: &'a VersionedUpdateStatements,
        stmts_role: &'a RolesStatements,
    ) -> Self {
        Self {
            client,
            stmts_version,
            stmts_role,
        }
    }

    pub async fn add_role(&mut self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let update = match VersionedUpdate::new(self.client, self.stmts_version, user_id).await? {
            Some(update) => update,
            None => return Ok(None),
        };

        match self.stmts_role.add.execute(update.client(), &user_id, &role).await {
            Ok(_) => {}
            Err(err) if err.is_constraint("roles", "fkey_user_id") => {
                // user not found, deleted meanwhile
                return Ok(None);
            }
            Err(err) if err.is_constraint("roles", "roles_idx_user_id_role") => {
                // role already present, it's ok
                return Ok(Some(()));
            }
            Err(err) => return Err(err.into()),
        };

        update.finish().await?;
        Ok(Some(()))
    }

    pub async fn get_roles(&mut self, user_id: Uuid) -> Result<Option<Vec<String>>, IdentityError> {
        let roles = self.stmts_role.get.query_opt(&*self.client, &user_id).await?;
        if let Some(roles) = roles {
            Ok(Some(roles.roles))
        } else {
            Ok(None)
        }
    }

    pub async fn delete_role(&mut self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let update = match VersionedUpdate::new(self.client, self.stmts_version, user_id).await? {
            Some(update) => update,
            None => return Ok(None),
        };

        self.stmts_role.delete.execute(update.client(), &user_id, &role).await?;

        update.finish().await?;
        Ok(Some(()))
    }
}
