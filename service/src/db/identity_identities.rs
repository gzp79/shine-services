use crate::db::{IdentityBuildError, IdentityError};
use bytes::BytesMut;
use chrono::{DateTime, Utc};
use shine_service::{
    pg_query,
    service::{PGClient, PGConnection, PGConvertError, PGErrorChecks, PGRawConnection, QueryBuilder, ToPGType},
};

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

#[derive(Clone, Debug)]
pub struct ExternalUserInfo {
    pub provider: String,
    pub provider_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
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
        SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created, i.data_version,
            e.provider, e.provider_id, e.name, e.email, e.linked
            FROM external_logins e, identities i
            WHERE e.user_id = i.user_id
                AND e.provider = $1
                AND e.provider_id = $2
    "#
);

pub struct IdentitiesStatements {
    insert_identity: InsertIdentity,
    insert_external_link: InsertExternalLogin,
    cascaded_delete: CascadedDelete,
    find_by_id: FindById,
    find_by_link: FindByLink,
}

impl IdentitiesStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            insert_identity: InsertIdentity::new(client).await?,
            insert_external_link: InsertExternalLogin::new(client).await?,
            cascaded_delete: CascadedDelete::new(client).await?,
            find_by_id: FindById::new(client).await?,
            find_by_link: FindByLink::new(client).await?,
        })
    }
}

/// Identities Data Access Object.
pub struct IdentitiesDAO<'a, T>
where
    T: PGRawConnection,
{
    client: &'a PGConnection<T>,
    stmts_identities: &'a IdentitiesStatements,
}

impl<'a, T> IdentitiesDAO<'a, T>
where
    T: PGRawConnection,
{
    pub fn new(client: &'a PGConnection<T>, stmts_identities: &'a IdentitiesStatements) -> Self {
        Self {
            client,
            stmts_identities,
        }
    }

    pub async fn create_user(
        &mut self,
        user_id: Uuid,
        user_name: &str,
        email: Option<&str>,
        //external_login: Option<&ExternalUserInfo>,
    ) -> Result<Identity, IdentityError> {
        //let email = email.map(|e| e.normalize_email());
        //let transaction = self.client.transaction().await?;

        let created = match self
            .stmts_identities
            .insert_identity
            .query_one(self.client, &user_id, &IdentityKind::User, &user_name, &email)
            .await
        {
            Ok(created) => created,
            Err(err) if err.is_constraint("identities", "identities_pkey") => {
                log::info!("Conflicting user id: {}, rolling back user creation", user_id);
                //transaction.rollback().await?;
                return Err(IdentityError::UserIdConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_name") => {
                log::info!("Conflicting name: {}, rolling back user creation", user_name);
                //transaction.rollback().await?;
                return Err(IdentityError::NameConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_email") => {
                log::info!("Conflicting email: {}, rolling back user creation", user_id);
                //transaction.rollback().await?;
                return Err(IdentityError::LinkEmailConflict);
            }
            Err(err) => {
                return Err(IdentityError::DBError(err.into()));
            }
        };

        /*if let Some(external_user) = external_login {
            if let Err(err) = self
                .stmts_identities
                .insert_external_link
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

        transaction.commit().await?;*/
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

    pub async fn find_by_id(&mut self, user_id: Uuid) -> Result<Option<Identity>, IdentityError> {
        Ok(self
            .stmts_identities
            .find_by_id
            .query_opt(self.client, &user_id)
            .await?
            .map(|row| Identity {
                id: row.user_id,
                kind: row.kind,
                name: row.name,
                email: row.email,
                is_email_confirmed: row.is_email_confirmed,
                created: row.created,
                version: row.version,
            }))
    }

    pub async fn find_by_external_link(
        &mut self,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<Identity>, IdentityError> {
        Ok(self
            .stmts_identities
            .find_by_link
            .query_opt(self.client, &provider, &provider_id)
            .await?
            .map(|row| Identity {
                id: row.user_id,
                kind: row.kind,
                name: row.name,
                email: row.email,
                is_email_confirmed: row.is_email_confirmed,
                created: row.created,
                version: row.version,
            }))
    }

    pub async fn search(&self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        log::info!("{search:?}");
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
        let rows = self.client.query(&stmt, &params).await?;

        let identities = rows.into_iter().map(into_identity).collect::<Result<Vec<_>, _>>()?;
        Ok(identities)
    }

    pub async fn cascaded_delete(&mut self, user_id: Uuid) -> Result<(), IdentityError> {
        self.stmts_identities
            .cascaded_delete
            .execute(self.client, &user_id)
            .await?;
        Ok(())
    }

    pub async fn link_user(&mut self, user_id: Uuid, external_user: &ExternalUserInfo) -> Result<(), IdentityError> {
        match self
            .stmts_identities
            .insert_external_link
            .query_one(
                self.client,
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
}
