use crate::repositories::identity::{IdentityBuildError, IdentityError, Roles};
use postgres_from_row::FromRow;
use shine_infra::{
    db::{DBError, PGClient, PGErrorChecks},
    pg_query,
};
use tracing::instrument;
use uuid::Uuid;

use super::{PgIdentityDbContext, PgVersionedUpdate};

pg_query!( AddUserRole =>
    in = user_id: Uuid, role: &str;
    out = UserRolesRow;
    sql = r#"
         WITH inserted AS (
            INSERT INTO roles (user_id, role) 
            VALUES ($1, $2)
            ON CONFLICT (user_id, role) DO NOTHING
        )
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
    out = UserRolesRow;
    sql = r#"
        WITH deleted AS (
            DELETE FROM roles
            WHERE user_id = $1 AND role = $2
        )
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

#[derive(Clone)]
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

impl Roles for PgIdentityDbContext<'_> {
    #[instrument(skip(self))]
    async fn add_role(&mut self, user_id: Uuid, role: &str) -> Result<Option<Vec<String>>, IdentityError> {
        let update = match PgVersionedUpdate::new(&mut self.client, &self.stmts_version, user_id).await? {
            Some(update) => update,
            None => return Ok(None),
        };

        log::debug!("Adding role {} to user {}", role, user_id);
        let row = match self
            .stmts_roles
            .add
            .query_opt(update.transaction(), &user_id, &role)
            .await
        {
            Ok(roles) => roles,
            Err(err) if err.is_constraint("roles", "fkey_user_id") => {
                // user not found, deleted meanwhile
                return Ok(None);
            }
            Err(err) => return Err(DBError::from(err).into()),
        };

        update.finish().await?;

        if let Some(roles) = row {
            Ok(Some(roles.roles))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip(self))]
    async fn get_roles(&mut self, user_id: Uuid) -> Result<Option<Vec<String>>, IdentityError> {
        let row = self
            .stmts_roles
            .get
            .query_opt(&self.client, &user_id)
            .await
            .map_err(DBError::from)?;
        if let Some(roles) = row {
            Ok(Some(roles.roles))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip(self))]
    async fn delete_role(&mut self, user_id: Uuid, role: &str) -> Result<Option<Vec<String>>, IdentityError> {
        let update = match PgVersionedUpdate::new(&mut self.client, &self.stmts_version, user_id).await? {
            Some(update) => update,
            None => return Ok(None),
        };

        let row = self
            .stmts_roles
            .delete
            .query_opt(update.transaction(), &user_id, &role)
            .await
            .map_err(DBError::from)?;

        update.finish().await?;

        if let Some(roles) = row {
            Ok(Some(roles.roles))
        } else {
            Ok(None)
        }
    }
}
