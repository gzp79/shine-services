use crate::db::{DBError, DBPool, PGError};
use bytes::BytesMut;
use chrono::{DateTime, Duration, Utc};
use shine_service::{
    pg_prepared_statement,
    service::{PGConnectionPool, PGErrorChecks, QueryBuilder},
};
use std::sync::Arc;
use thiserror::Error as ThisError;
use tokio_postgres::{
    types::{accepts, to_sql_checked, FromSql, IsNull, ToSql, Type},
    Row,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum IdentityKind {
    User,
    Studio,
}

impl ToSql for IdentityKind {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, PGError> {
        let value = match self {
            IdentityKind::User => 1_i16,
            IdentityKind::Studio => 2_i16,
        };
        value.to_sql(ty, out)
    }

    accepts!(INT2);
    to_sql_checked!();
}

impl<'a> FromSql<'a> for IdentityKind {
    fn from_sql(ty: &Type, raw: &[u8]) -> Result<IdentityKind, PGError> {
        let value = i16::from_sql(ty, raw)?;
        match value {
            1 => Ok(IdentityKind::User),
            2 => Ok(IdentityKind::Studio),
            _ => Err(PGError::from("Invalid value for IdentityKind")),
        }
    }

    accepts!(INT2);
}

#[derive(Debug)]

pub struct Identity {
    pub user_id: Uuid,
    pub kind: IdentityKind,
    pub name: String,
    pub email: Option<String>,
    pub is_email_confirmed: bool,
    pub creation: DateTime<Utc>,
}

impl Identity {
    fn from_row(row: &Row) -> Result<Self, IdentityError> {
        Ok(Self {
            user_id: row.try_get(0)?,
            kind: row.try_get(1)?,
            name: row.try_get(2)?,
            email: row.try_get(3)?,
            is_email_confirmed: row.try_get(4)?,
            creation: row.try_get(5)?,
        })
    }
}

#[derive(Debug)]
pub struct ExternalLoginInfo {
    pub provider: String,
    pub provider_id: String,
}

#[derive(Debug)]
pub struct LoginTokenInfo {
    pub user_id: Uuid,
    pub token: String,
    pub created_at: DateTime<Utc>,
    pub expire_at: DateTime<Utc>,
    pub is_expired: bool,
}

impl LoginTokenInfo {
    fn from_find_row(row: &Row) -> Result<Self, IdentityError> {
        Ok(Self {
            user_id: row.try_get(0)?,
            token: row.try_get(6)?,
            created_at: row.try_get(7)?,
            expire_at: row.try_get(8)?,
            is_expired: row.try_get(9)?,
        })
    }
}

#[derive(Debug, ThisError)]
pub enum IdentityError {
    #[error("User id already taken")]
    UserIdConflict,
    #[error("Name already taken")]
    NameConflict,
    #[error("Email already linked to a user")]
    LinkEmailConflict,
    #[error("External id already linked to a user")]
    LinkProviderConflict,
    #[error("Failed to generate token")]
    TokenConflict,
    #[error(transparent)]
    DBError(#[from] DBError),
}

impl From<tokio_postgres::Error> for IdentityError {
    fn from(err: tokio_postgres::Error) -> Self {
        Self::DBError(err.into())
    }
}

/// Identity query options
#[derive(Debug)]
pub enum FindIdentity<'a> {
    UserId(Uuid),
    Email(&'a str),
    Name(&'a str),
    ExternalLogin(&'a ExternalLoginInfo),
    Token(&'a str),
}

#[derive(Debug)]
pub enum SearchIdentityOrder {
    UserId(Option<Uuid>),
    Email(Option<(String, Uuid)>),
    Name(Option<(String, Uuid)>),
}

#[derive(Debug)]
pub struct SearchIdentity<'a> {
    pub order: SearchIdentityOrder,
    pub count: Option<usize>,

    pub user_ids: Option<&'a [Uuid]>,
    pub emails: Option<&'a [String]>,
    pub names: Option<&'a [String]>,
}

pg_prepared_statement!( InsertIdentity => r#"
    INSERT INTO identities (user_id, kind, created, name, email) 
        VALUES ($1, $2, now(), $3, $4)
        RETURNING created
"#, [UUID, INT2, VARCHAR, VARCHAR] );

pg_prepared_statement!( InsertToken => r#"
    INSERT INTO login_tokens (user_id, token, created, expire) 
        VALUES ($1, $2, now(), now() + $3 * interval '1 seconds')
    RETURNING created, expire
"#, [UUID, VARCHAR, INT4] );

pg_prepared_statement!( InsertExternalLogin => r#"
    INSERT INTO external_logins (user_id, provider, provider_id, linked) 
        VALUES ($1, $2, $3, now())
    RETURNING linked
"#, [UUID, VARCHAR, VARCHAR] );

pg_prepared_statement!( CascadedDelete => r#"
    -- DELETE FROM external_logins WHERE user_id = $1; fkey constraint shall trigger a cascaded delete
    DELETE FROM identities WHERE user_id = $1;
"#, [UUID] );

pg_prepared_statement!( FindById => r#"
    SELECT user_id, kind, name, email, email_confirmed, created 
        FROM identities
        WHERE user_id = $1
"#, [UUID] );

pg_prepared_statement!( FindByEmail => r#"
    SELECT user_id, kind, name, email, email_confirmed, created 
            FROM identities
            WHERE email = $1
"#, [VARCHAR] );

pg_prepared_statement!( FindByName => r#"
    SELECT user_id, kind, name, email, email_confirmed, created 
            FROM identities
            WHERE name = $1
"#, [VARCHAR] );

pg_prepared_statement!( FindByLink => r#"
    SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created,
           e.provider, e.provider_id, e.linked
        FROM external_logins e, identities i
        WHERE e.user_id = i.user_id
            AND e.provider = $1
            AND e.provider_id = $2
"#, [VARCHAR, VARCHAR] );

pg_prepared_statement!( FindByToken => r#"
    SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created,
           t.token, t.created, t.expire, t.expire < now() is_expired
        FROM login_tokens t, identities i
        WHERE t.user_id = i.user_id
            AND t.token = $1
"#, [VARCHAR] );

pg_prepared_statement!( DeleteToken => r#"
    DELETE FROM login_tokens WHERE user_id = $1 AND token = $2
"#, [UUID, VARCHAR] );

pg_prepared_statement!( DeleteAllTokens => r#"
    DELETE FROM login_tokens WHERE user_id = $1
"#, [UUID] );

pg_prepared_statement!( AddUserRole => r#"
    INSERT INTO roles (user_id, role) 
        VALUES ($1, $2)
"#, [UUID, VARCHAR] );

pg_prepared_statement!( GetUserRoles => r#"
    SELECT role from roles where user_id = $1
"#, [UUID] );

pg_prepared_statement!( DeleteUserRole => r#"
    DELETE FROM roles WHERE user_id = $1 AND role = $2
"#, [UUID, VARCHAR] );

#[derive(Debug, ThisError)]
pub enum IdentityBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}

impl From<tokio_postgres::Error> for IdentityBuildError {
    fn from(err: tokio_postgres::Error) -> Self {
        Self::DBError(err.into())
    }
}

struct Inner {
    postgres: PGConnectionPool,
    stmt_insert_identity: InsertIdentity,
    stmt_insert_external_link: InsertExternalLogin,
    stmt_insert_token: InsertToken,
    stmt_cascaded_delete: CascadedDelete,
    stmt_find_by_id: FindById,
    stmt_find_by_email: FindByEmail,
    stmt_find_by_name: FindByName,
    stmt_find_by_link: FindByLink,
    stmt_find_by_token: FindByToken,
    stmt_delete_token: DeleteToken,
    stmt_delete_all_tokens: DeleteAllTokens,
    stmt_add_role: AddUserRole,
    stmt_get_roles: GetUserRoles,
    stmt_delete_role: DeleteUserRole,
}

#[derive(Clone)]
pub struct IdentityManager(Arc<Inner>);

impl IdentityManager {
    pub async fn new(pool: &DBPool) -> Result<Self, IdentityBuildError> {
        let client = pool.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt_insert_identity = InsertIdentity::new(&client).await?;
        let stmt_insert_external_link = InsertExternalLogin::new(&client).await?;
        let stmt_insert_token = InsertToken::new(&client).await?;
        let stmt_cascaded_delete = CascadedDelete::new(&client).await?;
        let stmt_find_by_id = FindById::new(&client).await?;
        let stmt_find_by_email = FindByEmail::new(&client).await?;
        let stmt_find_by_name = FindByName::new(&client).await?;
        let stmt_find_by_link = FindByLink::new(&client).await?;
        let stmt_find_by_token = FindByToken::new(&client).await?;
        let stmt_delete_token = DeleteToken::new(&client).await?;
        let stmt_delete_all_tokens = DeleteAllTokens::new(&client).await?;
        let stmt_add_role = AddUserRole::new(&client).await?;
        let stmt_get_roles = GetUserRoles::new(&client).await?;
        let stmt_delete_role = DeleteUserRole::new(&client).await?;

        Ok(Self(Arc::new(Inner {
            postgres: pool.postgres.clone(),
            stmt_insert_identity,
            stmt_insert_external_link,
            stmt_insert_token,
            stmt_cascaded_delete,
            stmt_find_by_id,
            stmt_find_by_email,
            stmt_find_by_name,
            stmt_find_by_link,
            stmt_find_by_token,
            stmt_delete_token,
            stmt_delete_all_tokens,
            stmt_add_role,
            stmt_get_roles,
            stmt_delete_role,
        })))
    }

    pub async fn create_user(
        &self,
        user_id: Uuid,
        user_name: &str,
        email: Option<&str>,
        external_login: Option<&ExternalLoginInfo>,
    ) -> Result<Identity, IdentityError> {
        //let email = email.map(|e| e.normalize_email());
        let inner = &*self.0;

        let mut client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt_insert_identity = inner.stmt_insert_identity.get(&client).await?;
        let stmt_insert_external_link = inner.stmt_insert_external_link.get(&client).await?;

        let transaction = client.transaction().await?;

        let created_at: DateTime<Utc> = match transaction
            .query_one(
                &stmt_insert_identity,
                &[&user_id, &IdentityKind::User, &user_name, &email],
            )
            .await
        {
            Ok(row) => row.get(0),
            Err(err) if err.is_constraint("identities", "identities_pkey") => {
                log::info!("Conflicting user id: {}, rolling back user creation", user_id);
                transaction.rollback().await?;
                return Err(IdentityError::UserIdConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_name") => {
                log::info!("Conflicting name: {}, rolling back user creation", user_name);
                transaction.rollback().await?;
                return Err(IdentityError::NameConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_email") => {
                log::info!("Conflicting email: {}, rolling back user creation", user_id);
                transaction.rollback().await?;
                return Err(IdentityError::LinkEmailConflict);
            }
            Err(err) => {
                return Err(IdentityError::DBError(err.into()));
            }
        };

        if let Some(external_login) = external_login {
            if let Err(err) = transaction
                .execute(
                    &stmt_insert_external_link,
                    &[&user_id, &external_login.provider, &external_login.provider_id],
                )
                .await
            {
                if err.is_constraint("external_logins", "idx_provider_provider_id") {
                    transaction.rollback().await?;
                    return Err(IdentityError::LinkProviderConflict);
                } else {
                    return Err(IdentityError::DBError(err.into()));
                }
            };
        }

        transaction.commit().await?;
        Ok(Identity {
            user_id,
            name: user_name.to_owned(),
            email: email.map(String::from),
            is_email_confirmed: false,
            kind: IdentityKind::User,
            creation: created_at,
        })
    }

    pub async fn find(&self, find: FindIdentity<'_>) -> Result<Option<Identity>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;

        let identity = match find {
            FindIdentity::UserId(id) => {
                let stmt = inner.stmt_find_by_id.get(&client).await?;
                client.query_opt(&stmt, &[&id]).await?
            }
            FindIdentity::Email(email) => {
                let stmt = inner.stmt_find_by_email.get(&client).await?;
                client.query_opt(&stmt, &[&email]).await?
            }
            FindIdentity::Name(name) => {
                let stmt = inner.stmt_find_by_name.get(&client).await?;
                client.query_opt(&stmt, &[&name]).await?
            }
            FindIdentity::ExternalLogin(external_login) => {
                let stmt = inner.stmt_find_by_link.get(&client).await?;
                client
                    .query_opt(&stmt, &[&external_login.provider, &external_login.provider_id])
                    .await?
            }
            FindIdentity::Token(token) => {
                let stmt = inner.stmt_find_by_token.get(&client).await?;
                client.query_opt(&stmt, &[&token]).await?
            }
        };

        if let Some(identity) = identity {
            Ok(Some(Identity::from_row(&identity)?))
        } else {
            Ok(None)
        }
    }

    pub async fn search(&self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        const MAX_COUNT: usize = 100;

        log::info!("{search:?}");

        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;

        let mut builder = QueryBuilder::new("SELECT user_id, kind, name, created FROM identities");

        if let Some(user_ids) = &search.user_ids {
            builder.and_where(|b| format!("user_id = ANY(${b})"), [user_ids]);
        }

        if let Some(names) = &search.names {
            builder.and_where(|b| format!("name = ANY(${b})"), [names]);
        }

        if let Some(emails) = &search.emails {
            builder.and_where(|b| format!("email = ANY(${b})"), [emails]);
        }

        match &search.order {
            SearchIdentityOrder::UserId(start) => {
                if let Some(user_id) = start {
                    builder.and_where(|b| format!("user_id > ${b}"), [user_id]);
                }
            }
            SearchIdentityOrder::Email(start) => {
                if let Some((email, user_id)) = start {
                    builder.and_where(
                        |b1, b2| format!("(email > ${b1} OR (email == ${b1} AND user_id > ${b2}))"),
                        [email, user_id],
                    );
                }
                builder.order_by("email");
            }
            SearchIdentityOrder::Name(start) => {
                if let Some((name, user_id)) = start {
                    builder.and_where(
                        |b1, b2| format!("(name > ${b1} OR (name == ${b1} AND user_id > ${b2}))"),
                        [name, user_id],
                    );
                }
                builder.order_by("name");
            }
        };
        builder.order_by("user_id");

        let count = usize::min(MAX_COUNT, search.count.unwrap_or(MAX_COUNT));
        builder.limit(count);

        let (stmt, params) = builder.build();
        log::info!("{stmt:?}");
        let rows = client.query(&stmt, &params).await?;

        let identities = rows
            .into_iter()
            .map(|row| Identity::from_row(&row))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(identities)
    }

    pub async fn cascaded_delete(&self, user_id: Uuid) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt = inner.stmt_cascaded_delete.get(&client).await?;

        client
            .execute(&stmt, &[&user_id])
            .await
            .map_err(|err| IdentityError::DBError(err.into()))?;
        Ok(())
    }

    pub async fn link_user(&self, user_id: Uuid, external_login: &ExternalLoginInfo) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt_insert_external_link = inner.stmt_insert_external_link.get(&client).await?;

        match client
            .execute(
                &stmt_insert_external_link,
                &[&user_id, &external_login.provider, &external_login.provider_id],
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.is_constraint("external_logins", "idx_provider_provider_id") {
                    Err(IdentityError::LinkProviderConflict)
                } else {
                    Err(IdentityError::DBError(err.into()))
                }
            }
        }
    }

    /*pub async fn unlink_user(&self, user_id: Uuid, external_login: &ExternalLogin) -> Result<(), IdentityError> {
        todo!()
    }

    pub async fn get_links(&self, user_id: Uuid) -> Result<Vec<ExternalLogin>, IdentityError> {
        todo!()
    }*/

    pub async fn create_token(
        &self,
        user_id: Uuid,
        token: &str,
        duration: &Duration,
    ) -> Result<LoginTokenInfo, IdentityError> {
        let inner = &*self.0;

        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt = inner.stmt_insert_token.get(&client).await?;

        let duration = duration.num_seconds() as i32;
        assert!(duration > 0);
        let (created_at, expire_at): (DateTime<Utc>, DateTime<Utc>) =
            match client.query_one(&stmt, &[&user_id, &token, &duration]).await {
                Ok(row) => (row.get(0), row.get(1)),
                Err(err) if err.is_constraint("login_tokens", "idx_token") => {
                    return Err(IdentityError::TokenConflict);
                }
                Err(err) => {
                    return Err(IdentityError::DBError(err.into()));
                }
            };

        Ok(LoginTokenInfo {
            user_id,
            token: token.to_owned(),
            created_at,
            expire_at,
            is_expired: false,
        })
    }

    pub async fn find_token(&self, token: &str) -> Result<Option<(Identity, LoginTokenInfo)>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;

        let stmt = inner.stmt_find_by_token.get(&client).await?;
        let row = client.query_opt(&stmt, &[&token]).await?;

        if let Some(row) = row {
            let identity = Identity::from_row(&row)?;
            let token_info = LoginTokenInfo::from_find_row(&row)?;
            Ok(Some((identity, token_info)))
        } else {
            Ok(None)
        }
    }

    pub async fn update_token(&self, token: &str) -> Result<LoginTokenInfo, IdentityError> {
        // todo:
        // - update expiration
        // - update last use

        // workaround while update is not implemented
        Ok(self.find_token(token).await?.ok_or(IdentityError::TokenConflict)?.1)
    }

    pub async fn delete_token(&self, user_id: Uuid, token: &str) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt = inner.stmt_delete_token.get(&client).await?;

        client.execute(&stmt, &[&user_id, &token]).await?;
        Ok(())
    }

    pub async fn delete_all_tokens(&self, user_id: Uuid) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt = inner.stmt_delete_all_tokens.get(&client).await?;

        client.execute(&stmt, &[&user_id]).await?;
        Ok(())
    }

    pub async fn add_role(&self, user_id: Uuid, role: &str) -> Result<(), IdentityError> {
        let inner = &*self.0;

        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt = inner.stmt_add_role.get(&client).await?;

        match client.execute(&stmt, &[&user_id, &role]).await {
            Ok(_) => Ok(()),
            Err(err) if err.is_constraint("roles", "idx_user_idt_role") => {
                // role already present, it's ok
                Ok(())
            }
            Err(err) => Err(IdentityError::DBError(err.into())),
        }
    }

    pub async fn get_roles(&self, user_id: Uuid) -> Result<Vec<String>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;

        let stmt = inner.stmt_get_roles.get(&client).await?;
        let rows = client.query(&stmt, &[&user_id]).await?;

        let roles = rows.into_iter().map(|row| row.get::<_, String>(0)).collect();
        Ok(roles)
    }

    pub async fn delete_role(&self, user_id: Uuid, role: &str) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt = inner.stmt_delete_role.get(&client).await?;

        client.execute(&stmt, &[&user_id, &role]).await?;
        Ok(())
    }
}
