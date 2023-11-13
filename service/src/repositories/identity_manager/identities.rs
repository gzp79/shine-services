use crate::repositories::{IdentityBuildError, IdentityError};
use bytes::BytesMut;
use chrono::{DateTime, Utc};
use shine_service::{
    pg_query,
    service::{PGClient, PGConnection, PGConvertError, PGErrorChecks, PGRawConnection, ToPGType},
};
use tokio_postgres::types::{accepts, to_sql_checked, FromSql, IsNull, ToSql, Type};
use uuid::Uuid;

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

pub struct IdentitiesStatements {
    insert_identity: InsertIdentity,
    cascaded_delete: CascadedDelete,
    find_by_id: FindById,
}

impl IdentitiesStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            insert_identity: InsertIdentity::new(client).await?,
            cascaded_delete: CascadedDelete::new(client).await?,
            find_by_id: FindById::new(client).await?,
        })
    }
}

/// Identities Data Access Object.
pub struct Identities<'a, T>
where
    T: PGRawConnection,
{
    client: &'a PGConnection<T>,
    stmts_identities: &'a IdentitiesStatements,
}

impl<'a, T> Identities<'a, T>
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
    ) -> Result<Identity, IdentityError> {
        //let email = email.map(|e| e.normalize_email());

        let created = match self
            .stmts_identities
            .insert_identity
            .query_one(self.client, &user_id, &IdentityKind::User, &user_name, &email)
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
                return Err(IdentityError::LinkEmailConflict);
            }
            Err(err) => {
                return Err(IdentityError::DBError(err.into()));
            }
        };

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

    pub async fn cascaded_delete(&mut self, user_id: Uuid) -> Result<(), IdentityError> {
        self.stmts_identities
            .cascaded_delete
            .execute(self.client, &user_id)
            .await?;
        Ok(())
    }
}
