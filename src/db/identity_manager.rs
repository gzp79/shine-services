use crate::db::DBError;
use bytes::BytesMut;
use chrono::{DateTime, Duration, Utc};
use shine_service::{
    pg_query,
    service::{PGConnectionPool, PGConvertError, PGError, PGErrorChecks, QueryBuilder, ToPGType},
};
use std::sync::Arc;
use thiserror::Error as ThisError;
use tokio_postgres::{
    types::{accepts, to_sql_checked, FromSql, IsNull, ToSql, Type},
    Row,
};
use uuid::Uuid;

pub const MAX_SEARCH_COUNT: usize = 100;

#[derive(Debug, Clone, Copy)]
pub enum IdentityKind {
    User,
    Studio,
}

impl ToSql for IdentityKind {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, PGConvertError> {
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
    fn from_sql(ty: &Type, raw: &[u8]) -> Result<IdentityKind, PGConvertError> {
        let value = i16::from_sql(ty, raw)?;
        match value {
            1 => Ok(IdentityKind::User),
            2 => Ok(IdentityKind::Studio),
            _ => Err(PGConvertError::from("Invalid value for IdentityKind")),
        }
    }

    accepts!(INT2);
}

impl ToPGType for IdentityKind {
    const PG_TYPE: Type = Type::INT2;
}

#[derive(Clone, Debug)]

pub struct Identity {
    pub id: Uuid,
    pub kind: IdentityKind,
    pub name: String,
    pub email: Option<String>,
    pub is_email_confirmed: bool,
    pub created: DateTime<Utc>,
    pub version: i32,
}

impl From<FindByIdRow> for Identity {
    fn from(value: FindByIdRow) -> Self {
        Self {
            id: value.user_id,
            kind: value.kind,
            name: value.name,
            email: value.email,
            is_email_confirmed: value.is_email_confirmed,
            created: value.created,
            version: value.version,
        }
    }
}

impl From<FindByLinkRow> for Identity {
    fn from(value: FindByLinkRow) -> Self {
        Self {
            id: value.user_id,
            kind: value.kind,
            name: value.name,
            email: value.email,
            is_email_confirmed: value.is_email_confirmed,
            created: value.created,
            version: value.version,
        }
    }
}

impl From<FindByTokenRow> for (Identity, LoginTokenInfo) {
    fn from(value: FindByTokenRow) -> Self {
        let token = LoginTokenInfo {
            user_id: value.user_id,
            token: value.token,
            created: value.token_created,
            expire: value.token_expire,
            fingerprint: value.token_fingerprint,
            kind: value.token_kind,
            is_expired: value.token_is_expired,
        };
        let identity = Identity {
            id: value.user_id,
            kind: value.kind,
            name: value.name,
            email: value.email,
            is_email_confirmed: value.is_email_confirmed,
            created: value.created,
            version: value.version,
        };
        (identity, token)
    }
}

#[derive(Clone, Debug)]
pub struct ExternalUserInfo {
    pub provider: String,
    pub provider_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum TokenKind {
    SingleAccess,
    Persistent,
    AutoRenewal,
}

impl ToSql for TokenKind {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, PGConvertError> {
        let value = match self {
            TokenKind::SingleAccess => 1_i16,
            TokenKind::Persistent => 2_i16,
            TokenKind::AutoRenewal => 3_i16,
        };
        value.to_sql(ty, out)
    }

    accepts!(INT2);
    to_sql_checked!();
}

impl<'a> FromSql<'a> for TokenKind {
    fn from_sql(ty: &Type, raw: &[u8]) -> Result<TokenKind, PGConvertError> {
        let value = i16::from_sql(ty, raw)?;
        match value {
            1 => Ok(TokenKind::SingleAccess),
            2 => Ok(TokenKind::Persistent),
            3 => Ok(TokenKind::AutoRenewal),
            _ => Err(PGConvertError::from("Invalid value for TokenKind")),
        }
    }

    accepts!(INT2);
}

impl ToPGType for TokenKind {
    const PG_TYPE: Type = Type::INT2;
}

#[derive(Debug)]
pub struct LoginTokenInfo {
    pub user_id: Uuid,
    pub token: String,
    pub created: DateTime<Utc>,
    pub expire: DateTime<Utc>,
    pub is_expired: bool,

    pub kind: TokenKind,
    pub fingerprint: Option<String>,
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
    #[error("Operation failed with conflict, no change was made")]
    UpdateConflict,
    #[error(transparent)]
    DBError(#[from] DBError),
}

impl From<PGError> for IdentityError {
    fn from(err: PGError) -> Self {
        Self::DBError(err.into())
    }
}

/// Identity query options
#[derive(Debug)]
pub enum FindIdentity<'a> {
    UserId(Uuid),
    ExternalProvider(&'a str, &'a str),
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

pg_query!( InsertIdentity =>
    in = user_id: Uuid, kind: IdentityKind, name: &str, email: Option<&str>;
    out = created: DateTime<Utc>;
    sql = r#"
        INSERT INTO identities (user_id, kind, created, name, email) 
            VALUES ($1, $2, now(), $3, $4)
            RETURNING created
    "#
);

pg_query!( InsertToken =>
    in = user_id: Uuid, token: &str, fingerprint: Option<&str>, kind: TokenKind, expire_s: i32;
    out = InsertTokenRow{
        created: DateTime<Utc>,
        expire: DateTime<Utc>
    };
    sql =  r#"
        INSERT INTO login_tokens (user_id, token, created, fingerprint, kind, expire) 
            VALUES ($1, $2, now(), $3, $4, now() + $5 * interval '1 seconds')
        RETURNING created, expire
    "#
);

pg_query!( InsertExternalLogin =>
    in = user_id: Uuid, provider: &str, provider_id: &str, name: Option<&str>, email: Option<&str>;
    out = linked: DateTime<Utc>;
    sql = r#"
        INSERT INTO external_logins (user_id, provider, provider_id, name, email, linked) 
            VALUES ($1, $2, $3, $4, $5, now())
        RETURNING linked
    "#
);

pg_query!( CascadedDelete =>
    in = user_id: Uuid;
    sql = r#"
        -- DELETE FROM external_logins WHERE user_id = $1; fkey constraint shall trigger a cascaded delete
        DELETE FROM identities WHERE user_id = $1;
    "#
);

pg_query!( FindById =>
    in = user_id: Uuid;
    out = FindByIdRow{
        user_id: Uuid,
        kind: IdentityKind,
        name: String,
        email: Option<String>,
        is_email_confirmed: bool,
        created: DateTime<Utc>,
        version: i32
    };
    sql = r#"
        SELECT user_id, kind, name, email, email_confirmed, created, data_version
            FROM identities
            WHERE user_id = $1
    "#
);

pg_query!( FindByLink =>
    in = provider: &str, provider_id: &str;
    out = FindByLinkRow {
        user_id: Uuid,
        kind: IdentityKind,
        name: String,
        email: Option<String>,
        is_email_confirmed: bool,
        created: DateTime<Utc>,
        version: i32,
        provider: String,
        provider_id: String,
        external_name: Option<String>,
        external_email: Option<String>,
        linked: DateTime<Utc>
    };
    sql = r#"
        SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created, i.data_version
            e.provider, e.provider_id, e.name, e.email, e.linked
            FROM external_logins e, identities i
            WHERE e.user_id = i.user_id
                AND e.provider = $1
                AND e.provider_id = $2
    "#
);

pg_query!( FindByToken =>
    in = token: &str;
    out = FindByTokenRow{
        user_id: Uuid,
        kind: IdentityKind,
        name: String,
        email: Option<String>,
        is_email_confirmed: bool,
        created: DateTime<Utc>,
        version: i32,
        token: String,
        token_created: DateTime<Utc>,
        token_expire: DateTime<Utc>,
        token_fingerprint: Option<String>,
        token_kind: TokenKind,
        token_is_expired: bool
    };
    sql = r#"
        SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created, i.data_version
            t.token, t.created, t.expire, t.fingerprint, t.kind, t.expire < now() is_expired
            FROM login_tokens t, identities i
            WHERE t.user_id = i.user_id
                AND t.token = $1
    "#
);

pg_query!( DeleteToken =>
    in = user_id: Uuid, token: &str;
    sql = r#"
        DELETE FROM login_tokens WHERE user_id = $1 AND token = $2
    "#
);

pg_query!( DeleteAllTokens =>
    in = user_id: Uuid;
    sql = r#"
        DELETE FROM login_tokens WHERE user_id = $1
    "#
);

pg_query!( GetDataVersion =>
    in = user_id: Uuid;
    out = version: i32;
    sql = r#"
        SELECT data_version FROM identities WHERE user_id = $1
    "#
);

pg_query!( UpdateDataVersion =>
    in = user_id: Uuid, version: i32;
    sql = r#"
        UPDATE identities SET data_version = data_version + 1 WHERE user_id = $1 AND data_version = $2
    "#
);

pg_query!( AddUserRole =>
    in = user_id: Uuid, role: &str;
    sql = r#"
        INSERT INTO roles (user_id, role) 
            VALUES ($1, $2)
    "#
);

pg_query!( GetUserRoles =>
    in = user_id: Uuid;
    out = role: String;
    sql = r#"
        SELECT role from roles where user_id = $1
    "#
);

pg_query!( DeleteUserRole =>
    in = user_id: Uuid, role: &str;
    sql = r#"
        DELETE FROM roles WHERE user_id = $1 AND role = $2
    "#
);

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
    stmt_find_by_link: FindByLink,
    stmt_find_by_token: FindByToken,
    stmt_delete_token: DeleteToken,
    stmt_delete_all_tokens: DeleteAllTokens,
    stmt_get_version: GetDataVersion,
    stmt_update_version: UpdateDataVersion,
    stmt_add_role: AddUserRole,
    stmt_get_roles: GetUserRoles,
    stmt_delete_role: DeleteUserRole,
}

#[derive(Clone)]
pub struct IdentityManager(Arc<Inner>);

impl IdentityManager {
    pub async fn new(postgres: &PGConnectionPool) -> Result<Self, IdentityBuildError> {
        let client = postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt_insert_identity = InsertIdentity::new(&client).await?;
        let stmt_insert_external_link = InsertExternalLogin::new(&client).await?;
        let stmt_insert_token = InsertToken::new(&client).await?;
        let stmt_cascaded_delete = CascadedDelete::new(&client).await?;
        let stmt_find_by_id = FindById::new(&client).await?;
        let stmt_find_by_link = FindByLink::new(&client).await?;
        let stmt_find_by_token = FindByToken::new(&client).await?;
        let stmt_delete_token = DeleteToken::new(&client).await?;
        let stmt_delete_all_tokens: DeleteAllTokens = DeleteAllTokens::new(&client).await?;
        let stmt_get_version = GetDataVersion::new(&client).await?;
        let stmt_update_version = UpdateDataVersion::new(&client).await?;
        let stmt_add_role = AddUserRole::new(&client).await?;
        let stmt_get_roles = GetUserRoles::new(&client).await?;
        let stmt_delete_role = DeleteUserRole::new(&client).await?;

        Ok(Self(Arc::new(Inner {
            postgres: postgres.clone(),
            stmt_insert_identity,
            stmt_insert_external_link,
            stmt_insert_token,
            stmt_cascaded_delete,
            stmt_find_by_id,
            stmt_find_by_link,
            stmt_find_by_token,
            stmt_delete_token,
            stmt_delete_all_tokens,
            stmt_get_version,
            stmt_update_version,
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
        external_login: Option<&ExternalUserInfo>,
    ) -> Result<Identity, IdentityError> {
        //let email = email.map(|e| e.normalize_email());
        let inner = &*self.0;

        let mut client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let transaction = client.transaction().await?;

        let created = match inner
            .stmt_insert_identity
            .query_one(&transaction, &user_id, &IdentityKind::User, &user_name, &email)
            .await
        {
            Ok(created) => created,
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

        if let Some(external_user) = external_login {
            if let Err(err) = inner
                .stmt_insert_external_link
                .query_one(
                    &transaction,
                    &user_id,
                    &external_user.provider.as_str(),
                    &external_user.provider_id.as_str(),
                    &external_user.name.as_deref(),
                    &external_user.email.as_deref(),
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
            id: user_id,
            name: user_name.to_owned(),
            email: email.map(String::from),
            is_email_confirmed: false,
            kind: IdentityKind::User,
            created,
            version: 0,
        })
    }

    pub async fn find(&self, find: FindIdentity<'_>) -> Result<Option<Identity>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;

        Ok(match find {
            FindIdentity::UserId(id) => inner.stmt_find_by_id.query_opt(&client, &id).await?.map(Identity::from),

            FindIdentity::ExternalProvider(provider, provider_id) => inner
                .stmt_find_by_link
                .query_opt(&client, &provider, &provider_id)
                .await?
                .map(Identity::from),
        })
    }

    pub async fn search(&self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        log::info!("{search:?}");

        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;

        let mut builder = QueryBuilder::new(
            "SELECT user_id, kind, name, email, email_confirmed, created, data_version FROM identities",
        );

        fn into_identity(r: Row) -> Result<Identity, IdentityError> {
            Ok(Identity {
                id: r.try_get(0)?,
                kind: r.try_get(1)?,
                name: r.try_get(2)?,
                email: r.try_get(3)?,
                is_email_confirmed: r.try_get(4)?,
                created: r.try_get(5)?,
                version: r.try_get(6)?,
            })
        }

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

        let count = usize::min(MAX_SEARCH_COUNT, search.count.unwrap_or(MAX_SEARCH_COUNT));
        builder.limit(count);

        let (stmt, params) = builder.build();
        log::info!("{stmt:?}");
        let rows = client.query(&stmt, &params).await?;

        let identities = rows.into_iter().map(into_identity).collect::<Result<Vec<_>, _>>()?;
        Ok(identities)
    }

    pub async fn cascaded_delete(&self, user_id: Uuid) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        inner.stmt_cascaded_delete.execute(&client, &user_id).await?;
        Ok(())
    }

    pub async fn link_user(&self, user_id: Uuid, external_user: &ExternalUserInfo) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;

        match inner
            .stmt_insert_external_link
            .query_one(
                &client,
                &user_id,
                &external_user.provider.as_str(),
                &external_user.provider_id.as_str(),
                &external_user.name.as_deref(),
                &external_user.email.as_deref(),
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

    pub async fn create_token(
        &self,
        user_id: Uuid,
        token: &str,
        duration: &Duration,
        fingerprint: Option<&str>,
        kind: TokenKind,
    ) -> Result<LoginTokenInfo, IdentityError> {
        let inner = &*self.0;

        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;

        let duration = duration.num_seconds() as i32;
        assert!(duration > 2);
        let row = match inner
            .stmt_insert_token
            .query_one(&client, &user_id, &token, &fingerprint, &kind, &duration)
            .await
        {
            Ok(row) => row,
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
            created: row.created,
            expire: row.expire,
            fingerprint: fingerprint.map(|x| x.to_string()),
            kind,
            is_expired: false,
        })
    }

    pub async fn find_token(&self, token: &str) -> Result<Option<(Identity, LoginTokenInfo)>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        Ok(inner
            .stmt_find_by_token
            .query_opt(&client, &token)
            .await?
            .map(|r| r.into()))
    }

    pub async fn update_token(&self, token: &str, duration: &Duration) -> Result<LoginTokenInfo, IdentityError> {
        // issue#11:
        // - update expiration
        // - update last use

        let duration = duration.num_seconds() as i32;
        assert!(duration > 0);
        //issue#11:
        //  delete token where kind is SingleAccess
        //  update expire date where type is renewal
        //  delete token where expired

        // workaround while update is not implemented
        Ok(self.find_token(token).await?.ok_or(IdentityError::TokenConflict)?.1)
    }

    pub async fn delete_token(&self, user_id: Uuid, token: &str) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        inner.stmt_delete_token.execute(&client, &user_id, &token).await?;
        Ok(())
    }

    pub async fn delete_all_tokens(&self, user_id: Uuid) -> Result<(), IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        inner.stmt_delete_all_tokens.execute(&client, &user_id).await?;
        Ok(())
    }

    pub async fn add_role(&self, user_id: Uuid, role: &str) -> Result<(), IdentityError> {
        let inner = &*self.0;

        let mut client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let transaction = client.transaction().await?;
        let version: i32 = inner.stmt_get_version.query_one(&transaction, &user_id).await?;

        match inner.stmt_add_role.execute(&transaction, &user_id, &role).await {
            Ok(_) => {}
            Err(err) if err.is_constraint("roles", "roles_idx_user_id_role") => {
                // role already present, it's ok
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };

        if inner
            .stmt_update_version
            .execute(&transaction, &user_id, &version)
            .await?
            != 1
        {
            Err(IdentityError::UpdateConflict)
        } else {
            Ok(())
        }
    }

    pub async fn get_roles(&self, user_id: Uuid) -> Result<Vec<String>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let roles = inner.stmt_get_roles.query(&client, &user_id).await?;
        Ok(roles)
    }

    pub async fn delete_role(&self, user_id: Uuid, role: &str) -> Result<(), IdentityError> {
        let inner = &*self.0;

        let mut client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let transaction = client.transaction().await?;
        let version: i32 = inner.stmt_get_version.query_one(&transaction, &user_id).await?;

        inner.stmt_delete_role.execute(&transaction, &user_id, &role).await?;

        if inner
            .stmt_update_version
            .execute(&transaction, &user_id, &version)
            .await?
            != 1
        {
            Err(IdentityError::UpdateConflict)
        } else {
            Ok(())
        }
    }
}
