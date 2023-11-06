use crate::db::{IdentityBuildError, IdentityError, IdentityUOW, IdentityVersionStatements};
use shine_service::{
    pg_query,
    service::{PGErrorChecks as _, PGPooledConnection},
};
use uuid::Uuid;

pg_query!( AddUserRole =>
    in = user_id: Uuid, role: &str;
    sql = r#"
        INSERT INTO roles (user_id, role) 
            VALUES ($1, $2)
    "#
);

pg_query!( GetUserRoles =>
    in = user_id: Uuid;
    out = UserRoles {
        user_id: Option<Uuid>,
        roles: Vec<String>
    };
    sql = r#"
        SELECT i.user_id, 
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
    pub async fn new(client: &PGPooledConnection<'_>) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            add: AddUserRole::new(client).await?,
            get: GetUserRoles::new(client).await?,
            delete: DeleteUserRole::new(client).await?,
        })
    }
}

pub struct RolesDAO<'a> {
    client: PGPooledConnection<'a>,
    stmts_version: &'a IdentityVersionStatements,
    stmts_role: &'a RolesStatements,
}

impl<'a> RolesDAO<'a> {
    pub fn new(
        client: PGPooledConnection<'a>,
        stmts_version: &'a IdentityVersionStatements,
        stmts_role: &'a RolesStatements,
    ) -> Self {
        Self {
            client,
            stmts_version,
            stmts_role,
        }
    }

    pub async fn add_role(&mut self, user_id: Uuid, role: &str) -> Result<Option<()>, IdentityError> {
        let transaction = self.client.transaction().await?;
        let update = match IdentityUOW::new(transaction, self.stmts_version, user_id).await? {
            Some(update) => update,
            None => return Ok(None),
        };

        match self.stmts_role.add.execute(&update, &user_id, &role).await {
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
        let transaction = self.client.transaction().await?;
        let update = match IdentityUOW::new(transaction, self.stmts_version, user_id).await? {
            Some(update) => update,
            None => return Ok(None),
        };

        self.stmts_role.delete.execute(&update, &user_id, &role).await?;

        update.finish().await?;
        Ok(Some(()))
    }
}
