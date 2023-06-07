use crate::{
    db::{DBError, DBErrorChecks, DBPool, PGError},
    prepared_statement,
};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use thiserror::Error as ThisError;
use tokio_postgres::types::{private::BytesMut, Format::Binary, IsNull, ToSql, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum IdentityKind {
    User,
}

impl ToSql for IdentityKind {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, PGError>
    where
        Self: Sized,
    {
        todo!()
    }

    fn accepts(ty: &Type) -> bool
    where
        Self: Sized,
    {
        todo!()
    }

    fn to_sql_checked(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, PGError> {
        todo!()
    }

    fn encode_format(&self, _ty: &Type) -> tokio_postgres::types::Format {
        Binary
    }
}

#[derive(Debug)]
pub struct Identity {
    pub id: Uuid,
    pub name: String,
    pub kind: IdentityKind,
    pub creation: DateTime<Utc>,
}

#[derive(Debug, ThisError)]
pub enum IdentityError {
    #[error("User id already taken")]
    UserIdConflict,
    #[error("Name already taken")]
    NameConflict,
    #[error("External id already linked")]
    LinkConflict,
    #[error(transparent)]
    DBError(#[from] DBError),
}

#[derive(Debug, ThisError)]
pub enum IdentityBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}

#[derive(Debug)]
pub struct ExternalLogin {
    pub provider: String,
    pub provider_id: String,
}

prepared_statement!( InsertIdentity => r#"
    INSERT INTO identities (user_id, kind, created, name, email) 
        VALUES ($1, $2, now(), $3, $4)
        RETURNING created
"#, [UUID, INT2, VARCHAR, VARCHAR] );

prepared_statement!( InsertExternalLogin => r#"
    INSERT INTO external_logins (user_id, provider, provider_id, linked) 
        VALUES ($1, $2, $3, now())
        RETURNING linked
"#, [UUID, VARCHAR, VARCHAR] );

prepared_statement!( SelectByLink => r#"
    SELECT identities.user_id, kind, name, created 
        FROM external_logins, identities
        WHERE external_logins.user_id = identities.user_id
            AND external_logins.provider = $1
            AND external_logins.provider_id = $2
"#, [VARCHAR, VARCHAR] );

pub struct Inner {
    pool: DBPool,
    stmt_insert_identity: InsertIdentity,
    stmt_link_provider: InsertExternalLogin,
    stmt_find_by_link: SelectByLink,
}

#[derive(Clone)]
pub struct IdentityManager(Arc<Inner>);

impl IdentityManager {
    pub async fn new(pool: DBPool) -> Result<Self, IdentityBuildError> {
        let client = pool.get().await.map_err(DBError::from)?;
        let stmt_insert_identity = InsertIdentity::new(&client).await?;
        let stmt_link_provider = InsertExternalLogin::new(&client).await?;
        let stmt_find_by_link = SelectByLink::new(&client).await?;

        Ok(Self(Arc::new(Inner {
            pool: pool.clone(),
            stmt_insert_identity,
            stmt_link_provider,
            stmt_find_by_link,
        })))
    }

    pub async fn create_user(
        &self,
        user_id: Uuid,
        user_name: &str,
        email: Option<&str>,
        external_login: Option<&ExternalLogin>,
    ) -> Result<Identity, IdentityError> {
        //let email = email.map(|e| e.normalize_email());
        let inner = &*self.0;

        let mut client = inner.pool.get().await.map_err(DBError::from)?;
        let transaction = client.transaction().await.map_err(DBError::from)?;

        let created_at: DateTime<Utc> = match transaction
            .query_one(
                &*inner.stmt_insert_identity,
                &[&user_id, &IdentityKind::User, &user_name, &email],
            )
            .await
        {
            Ok(row) => row.get(0),
            Err(err) if err.is_constraint("identities", "idx_name") => {
                log::info!("Conflicting name: {}, rolling back user creation", user_name);
                transaction.rollback().await.map_err(DBError::from)?;
                return Err(IdentityError::NameConflict);
            }
            Err(err) if err.is_constraint("identities", "identities_pkey") => {
                log::info!("Conflicting user id: {}, rolling back user creation", user_id);
                transaction.rollback().await.map_err(DBError::from)?;
                return Err(IdentityError::UserIdConflict);
            }
            Err(err) => {
                return Err(IdentityError::DBError(err.into()));
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
                    return Err(IdentityError::LinkConflict);
                } else {
                    return Err(IdentityError::DBError(err.into()));
                }
            };
        }

        transaction.commit().await.map_err(DBError::from)?;
        Ok(Identity {
            id: user_id,
            name: user_name.to_owned(),
            kind: IdentityKind::User,
            creation: created_at,
        })
    }

    pub async fn find_user_by_id(&self, id: Uuid) -> Result<Option<Identity>, DBError> {
        todo!()
    }

    pub async fn find_user_by_email(&self, email: String) -> Result<Option<Identity>, DBError> {
        todo!()
    }

    pub async fn find_user_by_link(&self, external_login: &ExternalLogin) -> Result<Option<Identity>, DBError> {
        let inner = &*self.0;
        let client = inner.pool.get().await.map_err(DBError::from)?;

        let identity = client
            .query_one(
                &*inner.stmt_find_by_link,
                &[&external_login.provider, &external_login.provider_id],
            )
            .await?;
        /*let identity = (identity.get::<Uuid>(0), identity.get::<i32>(1), identity.get::<String>(2), identity.get::<DateTime<Utc>>(3));

        if let Some((user_id, kind, name, creation)) = identity {
            Ok(Some(Identity {
                id: user_id.0,
                kind: IdentityKind::try_from(kind)?,
                name,
                creation: creation.0,
            }))*/
        todo!()
        /*} else {
            Ok(None)
        }*/
    }

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
    }
}
