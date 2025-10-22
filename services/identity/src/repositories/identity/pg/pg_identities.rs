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
    in = user_id: Uuid, kind: IdentityKind, name: &str, encrypted_email: Option<&str>, email_hash: Option<&str>;
    out = created: DateTime<Utc>;
    sql = r#"
        INSERT INTO identities (user_id, kind, created, name, encrypted_email, email_hash)
            VALUES ($1, $2, now(), $3, $4, $5)
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
    encrypted_email: Option<String>,
    email_confirmed: bool,
    created: DateTime<Utc>,
}

pg_query!( FindById =>
    in = user_id: Uuid;
    out = IdentityRow;
    sql = r#"
        SELECT user_id, kind, name, encrypted_email, email_confirmed, created
            FROM identities
            WHERE user_id = $1
    "#
);

pg_query!( FindByEmailHash =>
    in = email_hash: &str;
    out = IdentityRow;
    sql = r#"
        SELECT user_id, kind, name, encrypted_email, email_confirmed, created
            FROM identities
            WHERE email_hash = $1
    "#
);

pg_query!( UpdateIdentity =>
    in = user_id: Uuid, user_name: Option<&str>, encrypted_email: Option<&str>, email_hash: Option<&str>, email_confirmed: Option<bool>;
    out = IdentityRow;
    sql = r#"
        UPDATE identities
            SET name = COALESCE($2, name),
                encrypted_email = COALESCE($3, encrypted_email),
                email_hash = COALESCE($4, email_hash),
                email_confirmed = COALESCE($5, email_confirmed)
            WHERE user_id = $1
        RETURNING user_id, kind, name, encrypted_email, email_confirmed, created
    "#
);

#[derive(Clone)]
pub struct PgIdentitiesStatements {
    insert_identity: InsertIdentity,
    cascaded_delete: CascadedDelete,
    find_by_id: FindById,
    find_by_email_hash: FindByEmailHash,
    update: UpdateIdentity,
}

impl PgIdentitiesStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            insert_identity: InsertIdentity::new(client).await.map_err(DBError::from)?,
            cascaded_delete: CascadedDelete::new(client).await.map_err(DBError::from)?,
            find_by_id: FindById::new(client).await.map_err(DBError::from)?,
            find_by_email_hash: FindByEmailHash::new(client).await.map_err(DBError::from)?,
            update: UpdateIdentity::new(client).await.map_err(DBError::from)?,
        })
    }
}

impl Identities for PgIdentityDbContext<'_> {
    #[instrument(skip(self))]
    async fn create_user(
        &mut self,
        user_id: Uuid,
        user_name: &str,
        email: Option<(&str, bool)>,
    ) -> Result<Identity, IdentityError> {
        let (encrypted_email, email_hash) = if let Some((email, _)) = email {
            let encrypted_email = self.crypto.encrypt(email)?;
            let email_hash = self.crypto.hash(email);
            (Some(encrypted_email), Some(email_hash))
        } else {
            (None, None)
        };

        let created = match self
            .stmts_identities
            .insert_identity
            .query_one(
                &self.client,
                &user_id,
                &IdentityKind::User,
                &user_name,
                &encrypted_email.as_deref(),
                &email_hash.as_deref(),
            )
            .await
        {
            Ok(created) => created,
            Err(err) if err.is_constraint("identities", "identities_pkey") => {
                log::info!("Conflicting user id: {user_id}, rolling back user creation");
                return Err(IdentityError::UserIdConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_name") => {
                log::info!("Conflicting name: {user_name}, rolling back user creation");
                return Err(IdentityError::NameConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_email_hash") => {
                log::info!("Conflicting email: {user_id}, rolling back user creation");
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
        })
    }

    #[instrument(skip(self))]
    async fn find_by_id(&mut self, id: Uuid) -> Result<Option<Identity>, IdentityError> {
        let row = self
            .stmts_identities
            .find_by_id
            .query_opt(&self.client, &id)
            .await
            .map_err(DBError::from)?;

        if let Some(row) = row {
            let email = if let Some(encrypted_email) = &row.encrypted_email {
                Some(self.crypto.decrypt(encrypted_email)?)
            } else {
                None
            };
            Ok(Some(Identity {
                id: row.user_id,
                kind: row.kind,
                name: row.name,
                email,
                is_email_confirmed: row.email_confirmed,
                created: row.created,
            }))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip(self))]
    async fn find_by_email(&mut self, email: &str) -> Result<Option<Identity>, IdentityError> {
        let email_hash = self.crypto.hash(email);
        let row = self
            .stmts_identities
            .find_by_email_hash
            .query_opt(&self.client, &email_hash.as_str())
            .await
            .map_err(DBError::from)?;

        if let Some(row) = row {
            let email = if let Some(encrypted_email) = &row.encrypted_email {
                Some(self.crypto.decrypt(encrypted_email)?)
            } else {
                None
            };
            Ok(Some(Identity {
                id: row.user_id,
                kind: row.kind,
                name: row.name,
                email,
                is_email_confirmed: row.email_confirmed,
                created: row.created,
            }))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip(self))]
    async fn update(
        &mut self,
        id: Uuid,
        name: Option<&str>,
        email: Option<(&str, bool)>,
    ) -> Result<Option<Identity>, IdentityError> {
        let (encrypted_email, email_hash) = if let Some((email, _)) = email {
            let encrypted_email = self.crypto.encrypt(email)?;
            let email_hash = self.crypto.hash(email);
            (Some(encrypted_email), Some(email_hash))
        } else {
            (None, None)
        };

        let identity_row = match self
            .stmts_identities
            .update
            .query_opt(
                &self.client,
                &id,
                &name,
                &encrypted_email.as_deref(),
                &email_hash.as_deref(),
                &email.map(|x| x.1),
            )
            .await
        {
            Ok(Some(row)) => row,
            Ok(None) => return Ok(None),
            Err(err) if err.is_constraint("identities", "idx_name") => {
                log::info!("Conflicting name: {name:?}, rolling back user update");
                return Err(IdentityError::NameConflict);
            }
            Err(err) if err.is_constraint("identities", "idx_email_hash") => {
                log::info!("Conflicting email: {email:?}, rolling back user update");
                return Err(IdentityError::EmailConflict);
            }
            Err(err) => return Err(DBError::from(err).into()),
        };

        let email = if let Some(encrypted_email) = &identity_row.encrypted_email {
            Some(self.crypto.decrypt(encrypted_email)?)
        } else {
            None
        };

        Ok(Some(Identity {
            id: identity_row.user_id,
            kind: identity_row.kind,
            name: identity_row.name,
            email,
            is_email_confirmed: identity_row.email_confirmed,
            created: identity_row.created,
        }))
    }

    #[instrument(skip(self))]
    async fn cascaded_delete(&mut self, id: Uuid) -> Result<(), IdentityError> {
        self.stmts_identities
            .cascaded_delete
            .execute(&self.client, &id)
            .await
            .map_err(DBError::from)?;
        Ok(())
    }
}
