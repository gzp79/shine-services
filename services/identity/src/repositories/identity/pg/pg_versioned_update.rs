use crate::repositories::identity::{IdentityBuildError, IdentityError};
use shine_infra::{
    db::{DBError, PGClient, PGPooledConnection, PGTransaction},
    pg_query,
};
use tracing::instrument;
use uuid::Uuid;

pg_query!( GetDataVersion =>
    in = user_id: Uuid;
    out = data_version: i32;
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

#[derive(Clone)]
pub struct PgVersionedUpdateStatements {
    get_version: GetDataVersion,
    update_version: UpdateDataVersion,
}

impl PgVersionedUpdateStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            get_version: GetDataVersion::new(client).await.map_err(DBError::from)?,
            update_version: UpdateDataVersion::new(client).await.map_err(DBError::from)?,
        })
    }
}

/// Creates a nested transaction that will update the version of the user's data with conflict detection.
pub struct PgVersionedUpdate<'a> {
    transaction: PGTransaction<'a>,
    stmts: &'a PgVersionedUpdateStatements,
    user_id: Uuid,
    version: i32,
}

impl<'a> PgVersionedUpdate<'a> {
    #[instrument(skip(client, stmts))]
    pub async fn new(
        client: &'a mut PGPooledConnection<'_>,
        stmts: &'a PgVersionedUpdateStatements,
        user_id: Uuid,
    ) -> Result<Option<PgVersionedUpdate<'a>>, IdentityError> {
        let transaction = client.transaction().await.map_err(DBError::from)?;
        let version: i32 = match stmts
            .get_version
            .query_opt(&transaction, &user_id)
            .await
            .map_err(DBError::from)?
        {
            Some(version) => version,
            None => return Ok(None),
        };

        Ok(Some(Self {
            transaction,
            stmts,
            user_id,
            version,
        }))
    }

    pub fn transaction(&self) -> &PGTransaction<'a> {
        &self.transaction
    }

    #[instrument(skip(self))]
    pub async fn finish(self) -> Result<(), IdentityError> {
        if self
            .stmts
            .update_version
            .execute(&self.transaction, &self.user_id, &self.version)
            .await
            .map_err(DBError::from)?
            != 1
        {
            self.transaction.rollback().await.map_err(DBError::from)?;
            Err(IdentityError::UpdateConflict { id: self.user_id })
        } else {
            self.transaction.commit().await.map_err(DBError::from)?;
            Ok(())
        }
    }
}
