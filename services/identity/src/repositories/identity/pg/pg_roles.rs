use crate::repositories::identity::{IdentityBuildError, IdentityError, Roles};
use postgres_from_row::FromRow;
use shine_infra::{
    db::{DBError, PGClient, PGErrorChecks},
    pg_query,
};
use tracing::instrument;
use uuid::Uuid;

use super::PgIdentityDbContext;

pg_query!( AddUserRole =>
    in = user_id: Uuid, role: &str;
    sql = r#"
         INSERT INTO roles (user_id, role) 
         VALUES ($1, $2)
         ON CONFLICT (user_id, role) DO NOTHING
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
        DELETE FROM roles
        WHERE user_id = $1 AND role = $2
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
    async fn add_role(
        &mut self,
        user_id: Uuid,
        role: &str,
    ) -> Result<Option<Vec<String>>, IdentityError> {
        log::debug!("Adding role {} to user {}", role, user_id);
        match self
            .stmts_roles
            .add
            .execute(&self.client, &user_id, &role)
            .await
        {
            Ok(_) => (),
            Err(err) if err.is_constraint("roles", "fkey_user_id") => {
                // user not found, deleted meanwhile
                return Ok(None);
            }
            Err(err) => return Err(DBError::from(err).into()),
        };
        self.get_roles(user_id).await
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
    async fn delete_role(
        &mut self,
        user_id: Uuid,
        role: &str,
    ) -> Result<Option<Vec<String>>, IdentityError> {
        self.stmts_roles
            .delete
            .execute(&self.client, &user_id, &role)
            .await
            .map_err(DBError::from)?;

        self.get_roles(user_id).await
    }
}
