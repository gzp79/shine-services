use crate::db::{DBError, DBPool, PGError};
use bytes::BytesMut;
use chrono::{DateTime, Utc};
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
    pub fn from_row(row: &Row) -> Result<Self, DBError> {
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
pub struct ExternalLogin {
    pub provider: String,
    pub provider_id: String,
}

#[derive(Debug, ThisError)]
pub enum CreateIdentityError {
    #[error("User id already taken")]
    UserIdConflict,
    #[error("Name already taken")]
    NameConflict,
    #[error("External id already linked")]
    LinkConflict,
    #[error(transparent)]
    DBError(#[from] DBError),
}

/// Identity query options
#[derive(Debug)]
pub enum FindIdentity<'a> {
    UserId(Uuid),
    Email(&'a str),
    Name(&'a str),
    ExternalLogin(&'a ExternalLogin),
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

pg_prepared_statement!( InsertExternalLogin => r#"
    INSERT INTO external_logins (user_id, provider, provider_id, linked) 
        VALUES ($1, $2, $3, now())
        RETURNING linked
"#, [UUID, VARCHAR, VARCHAR] );

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
    SELECT identities.user_id, kind, name, email, email_confirmed, created 
        FROM external_logins, identities
        WHERE external_logins.user_id = identities.user_id
            AND external_logins.provider = $1
            AND external_logins.provider_id = $2
"#, [VARCHAR, VARCHAR] );

#[derive(Debug, ThisError)]
pub enum IdentityBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}

struct Inner {
    postgres: PGConnectionPool,
    stmt_insert_identity: InsertIdentity,
    stmt_link_provider: InsertExternalLogin,
    stmt_find_by_id: FindById,
    stmt_find_by_email: FindByEmail,
    stmt_find_by_name: FindByName,
    stmt_find_by_link: FindByLink,
}

#[derive(Clone)]
pub struct IdentityManager(Arc<Inner>);

impl IdentityManager {
    pub async fn new(pool: &DBPool) -> Result<Self, IdentityBuildError> {
        let client = pool.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt_insert_identity = InsertIdentity::new(&client).await.map_err(DBError::from)?;
        let stmt_link_provider = InsertExternalLogin::new(&client).await.map_err(DBError::from)?;
        let stmt_find_by_id = FindById::new(&client).await.map_err(DBError::from)?;
        let stmt_find_by_email = FindByEmail::new(&client).await.map_err(DBError::from)?;
        let stmt_find_by_name = FindByName::new(&client).await.map_err(DBError::from)?;
        let stmt_find_by_link = FindByLink::new(&client).await.map_err(DBError::from)?;

        Ok(Self(Arc::new(Inner {
            postgres: pool.postgres.clone(),
            stmt_insert_identity,
            stmt_link_provider,
            stmt_find_by_id,
            stmt_find_by_email,
            stmt_find_by_name,
            stmt_find_by_link,
        })))
    }

    pub async fn create_user(
        &self,
        user_id: Uuid,
        user_name: &str,
        email: Option<&str>,
        external_login: Option<&ExternalLogin>,
    ) -> Result<Identity, CreateIdentityError> {
        //let email = email.map(|e| e.normalize_email());
        let inner = &*self.0;

        let mut client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let transaction = client.transaction().await.map_err(DBError::from)?;

        let created_at: DateTime<Utc> = match transaction
            .query_one(
                &*inner.stmt_insert_identity,
                &[&user_id, &IdentityKind::User, &user_name, &email],
            )
            .await
        {
            Ok(row) => row.get(0),
            Err(err) if err.is_constraint("identities", "identities_pkey") => {
                log::info!("Conflicting user id: {}, rolling back user creation", user_id);
                transaction.rollback().await.map_err(DBError::from)?;
                return Err(CreateIdentityError::UserIdConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_name") => {
                log::info!("Conflicting name: {}, rolling back user creation", user_name);
                transaction.rollback().await.map_err(DBError::from)?;
                return Err(CreateIdentityError::NameConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_email") => {
                log::info!("Conflicting email: {}, rolling back user creation", user_id);
                transaction.rollback().await.map_err(DBError::from)?;
                return Err(CreateIdentityError::LinkConflict);
            }
            Err(err) => {
                return Err(CreateIdentityError::DBError(err.into()));
            }
        };

        if let Some(external_login) = external_login {
            if let Err(err) = transaction
                .execute(
                    &*inner.stmt_link_provider,
                    &[&user_id, &external_login.provider, &external_login.provider_id],
                )
                .await
            {
                if err.is_constraint("external_logins", "idx_provider_provider_id") {
                    transaction.rollback().await.map_err(DBError::from)?;
                    return Err(CreateIdentityError::LinkConflict);
                } else {
                    return Err(CreateIdentityError::DBError(err.into()));
                }
            };
        }

        transaction.commit().await.map_err(DBError::from)?;
        Ok(Identity {
            user_id,
            name: user_name.to_owned(),
            email: email.map(String::from),
            is_email_confirmed: false,
            kind: IdentityKind::User,
            creation: created_at,
        })
    }

    pub async fn find(&self, find: FindIdentity<'_>) -> Result<Option<Identity>, DBError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;

        let identity = match find {
            FindIdentity::UserId(id) => client.query_opt(&*inner.stmt_find_by_id, &[&id]).await?,
            FindIdentity::Email(email) => client.query_opt(&*inner.stmt_find_by_email, &[&email]).await?,
            FindIdentity::Name(name) => client.query_opt(&*inner.stmt_find_by_name, &[&name]).await?,
            FindIdentity::ExternalLogin(external_login) => {
                client
                    .query_opt(
                        &*inner.stmt_find_by_link,
                        &[&external_login.provider, &external_login.provider_id],
                    )
                    .await?
            }
        };

        if let Some(identity) = identity {
            Ok(Some(Identity::from_row(&identity)?))
        } else {
            Ok(None)
        }
    }

    pub async fn search(&self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, DBError> {
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

        let identites = rows
            .into_iter()
            .map(|row| Identity::from_row(&row))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(identites)
    }

    /*
    pub async fn link_user(&self, user_id: Uuid, external_login: &ExternalLogin) -> Result<(), DBError> {
        /*let id_str = user_id.hyphenated().to_string();
        let link_response = sql_expr!(
            self.db_kind(),
            "INSERT INTO external_logins (user_id, provider, provider_id, linked)"
                + "VALUES(uuid(${&id_str}), ${&external_login.provider}, ${&external_login.provider_id}, ${expr::Now})"
                + "ON CONFLICT DO NOTHING"
                + "RETURNING 'ok'"
        )
        .to_query_as::<_, (String,)>()
        .fetch_optional(&self.pool)
        .await?;

        // check if link could be added
        if link_response.unwrap_or_default().0 == "ok" {
            Ok(())
        } else {
            Err(DBError::Conflict)
        }*/
        todo!()
    }

    pub async fn unlink_user(&self, user_id: Uuid, provider: String) -> Result<(), DBError> {
        todo!()
    }

    pub async fn get_linked_providers(&self, user_id: Uuid) -> Result<Vec<ExternalLogin>, DBError> {
        todo!()
    }*/
}
