use crate::repositories::{identity::roles::Roles, DBError, IdentityBuildError, IdentityError};
use postgres_from_row::FromRow;
use shine_service::{
    pg_query,
    service::{PGClient, PGErrorChecks},
};
use tracing::instrument;
use uuid::Uuid;

use super::{PgIdentityTransaction, PgVersionedUpdate};

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

pub struct PgRolesStatements {
    add: AddUserRole,
    get: GetUserRoles,
    delete: DeleteUserRole,
}

impl PgRolesStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            add: AddUserRole::new(client).await.map_err(DBError::from)?,
            get: GetUserRoles::new(client).await.map_err(DBError::from)?,
            delete: DeleteUserRole::new(client).await.map_err(DBError::from)?,
        })
    }
}

impl<'a> Roles for PgIdentityTransaction<'a> {
    #[instrument(skip(self))]
    async fn add_role(&mut self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let update = match PgVersionedUpdate::new(&mut self.transaction, &self.stmts_version, user_id).await? {
            Some(update) => update,
            None => return Ok(None),
        };

        log::debug!("Adding role {} to user {}", role, user_id);
        match self
            .stmts_roles
            .add
            .execute(update.transaction(), &user_id, &role)
            .await
        {
            Ok(_) => {}
            Err(err) if err.is_constraint("roles", "fkey_user_id") => {
                // user not found, deleted meanwhile
                return Ok(None);
            }
            Err(err) if err.is_constraint("roles", "roles_idx_user_id_role") => {
                // role already present, it's ok
                return Ok(Some(()));
            }
            Err(err) => return Err(DBError::from(err).into()),
        };

        update.finish().await?;
        Ok(Some(()))
    }

    #[instrument(skip(self))]
    async fn get_roles(&mut self, user_id: Uuid) -> Result<Option<Vec<String>>, IdentityError> {
        let roles = self
            .stmts_roles
            .get
            .query_opt(&self.transaction, &user_id)
            .await
            .map_err(DBError::from)?;
        if let Some(roles) = roles {
            Ok(Some(roles.roles))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip(self))]
    async fn delete_role(&mut self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let update = match PgVersionedUpdate::new(&mut self.transaction, &self.stmts_version, user_id).await? {
            Some(update) => update,
            None => return Ok(None),
        };

        self.stmts_roles
            .delete
            .execute(update.transaction(), &user_id, &role)
            .await
            .map_err(DBError::from)?;

        update.finish().await?;
        Ok(Some(()))
    }
}
