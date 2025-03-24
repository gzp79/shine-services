use crate::repositories::identity::{Identities, Identity, IdentityBuildError, IdentityError, IdentityKind};
use bytes::BytesMut;
use chrono::{DateTime, Utc};
use postgres_from_row::FromRow;
use shine_infra::{
    db::{DBError, PGClient, PGConvertError, PGErrorChecks, PGValueTypeINT2, ToPGType},
    pg_query,
};
use tokio_postgres::types::{accepts, to_sql_checked, FromSql, IsNull, ToSql, Type};
use tracing::instrument;
use uuid::Uuid;

use super::PgIdentityDbContext;

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

impl FromSql<'_> for IdentityKind {
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
    type PGValueType = PGValueTypeINT2;
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

pg_query!( CascadedDelete =>
    in = user_id: Uuid;
    sql = r#"
        -- DELETE FROM external_logins WHERE user_id = $1; fkey constraint shall trigger a cascaded delete
        DELETE FROM identities WHERE user_id = $1;
    "#
);

#[derive(FromRow)]
struct IdentityRow {
    user_id: Uuid,
    kind: IdentityKind,
    name: String,
    email: Option<String>,
    email_confirmed: bool,
    created: DateTime<Utc>,
    data_version: i32,
}

pg_query!( FindById =>
    in = user_id: Uuid;
    out = IdentityRow;
    sql = r#"
        SELECT user_id, kind, name, email, email_confirmed, created, data_version
            FROM identities
            WHERE user_id = $1
    "#
);

pg_query!( UpdateIdentity =>
    in = user_id: Uuid, user_name: Option<&str>, email: Option<&str>, email_confirmed: Option<bool>;
    out = IdentityRow;
    sql = r#"
        UPDATE identities
            SET name = COALESCE($2, name),
                email = COALESCE($3, email),
                email_confirmed = COALESCE($4, email_confirmed)
            WHERE user_id = $1
        RETURNING user_id, kind, name, email, email_confirmed, created, data_version
    "#
);

#[derive(Clone)]
pub struct PgIdentitiesStatements {
    insert_identity: InsertIdentity,
    cascaded_delete: CascadedDelete,
    find_by_id: FindById,
    update: UpdateIdentity,
}

impl PgIdentitiesStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            insert_identity: InsertIdentity::new(client).await.map_err(DBError::from)?,
            cascaded_delete: CascadedDelete::new(client).await.map_err(DBError::from)?,
            find_by_id: FindById::new(client).await.map_err(DBError::from)?,
            update: UpdateIdentity::new(client).await.map_err(DBError::from)?,
        })
    }
}

impl<'a> Identities for PgIdentityDbContext<'a> {
    #[instrument(skip(self))]
    async fn create_user(
        &mut self,
        user_id: Uuid,
        user_name: &str,
        email: Option<(&str, bool)>,
    ) -> Result<Identity, IdentityError> {
        //let email = email.map(|e| e.normalize_email());

        let created = match self
            .stmts_identities
            .insert_identity
            .query_one(
                &self.client,
                &user_id,
                &IdentityKind::User,
                &user_name,
                &email.map(|x| x.0),
            )
            .await
        {
            Ok(created) => created,
            Err(err) if err.is_constraint("identities", "identities_pkey") => {
                log::info!("Conflicting user id: {}, rolling back user creation", user_id);
                return Err(IdentityError::UserIdConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_name") => {
                log::info!("Conflicting name: {}, rolling back user creation", user_name);
                return Err(IdentityError::NameConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_email") => {
                log::info!("Conflicting email: {}, rolling back user creation", user_id);
                return Err(IdentityError::EmailConflict);
            }
            Err(err) => {
                return Err(IdentityError::DBError(err.into()));
            }
        };

        Ok(Identity {
            id: user_id,
            name: user_name.to_owned(),
            email: email.map(|x| x.0.to_owned()),
            is_email_confirmed: email.map(|x| x.1).unwrap_or(false),
            kind: IdentityKind::User,
            created,
            version: 0,
        })
    }

    #[instrument(skip(self))]
    async fn find_by_id(&mut self, id: Uuid) -> Result<Option<Identity>, IdentityError> {
        Ok(self
            .stmts_identities
            .find_by_id
            .query_opt(&self.client, &id)
            .await
            .map_err(DBError::from)?
            .map(|row| Identity {
                id: row.user_id,
                kind: row.kind,
                name: row.name,
                email: row.email,
                is_email_confirmed: row.email_confirmed,
                created: row.created,
                version: row.data_version,
            }))
    }

    #[instrument(skip(self))]
    async fn update(
        &mut self,
        id: Uuid,
        name: Option<&str>,
        email: Option<(&str, bool)>,
    ) -> Result<Option<Identity>, IdentityError> {
        //todo: use PgVersionedUpdate to trigger a session update for name and similar things
        let identity_row = match self
            .stmts_identities
            .update
            .query_opt(&self.client, &id, &name, &email.map(|x| x.0), &email.map(|x| x.1))
            .await
        {
            Ok(Some(row)) => row,
            Ok(None) => return Ok(None),
            Err(err) if err.is_constraint("identities", "idx_name") => {
                log::info!("Conflicting name: {:?}, rolling back user update", name);
                return Err(IdentityError::NameConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_email") => {
                log::info!("Conflicting email: {:?}, rolling back user update", email);
                return Err(IdentityError::EmailConflict);
            }
            Err(err) => return Err(DBError::from(err).into()),
        };

        Ok(Some(Identity {
            id: identity_row.user_id,
            kind: identity_row.kind,
            name: identity_row.name,
            email: identity_row.email,
            is_email_confirmed: identity_row.email_confirmed,
            created: identity_row.created,
            version: identity_row.data_version,
        }))
    }

    #[instrument(skip(self))]
    async fn cascaded_delete(&mut self, id: Uuid) -> Result<(), IdentityError> {
        self.stmts_identities
            .cascaded_delete
            .execute(&self.client, &id)
            .await
            .map_err(DBError::from)
            .map_err(DBError::from)?;
        Ok(())
    }
}
