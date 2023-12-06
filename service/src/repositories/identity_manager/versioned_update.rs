use crate::repositories::{IdentityBuildError, IdentityError};
use shine_service::{
    pg_query,
    service::{PGClient, PGConnection, PGRawConnection, PGTransaction},
};
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

pub struct VersionedUpdateStatements {
    get_version: GetDataVersion,
    update_version: UpdateDataVersion,
}

impl VersionedUpdateStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            get_version: GetDataVersion::new(client).await?,
            update_version: UpdateDataVersion::new(client).await?,
        })
    }
}

/// Unit of work for identity data. It keeps track of version and handle transaction with version check.
pub struct VersionedUpdate<'a> {
    transaction: PGTransaction<'a>,
    stmts: &'a VersionedUpdateStatements,
    user_id: Uuid,
    version: i32,
}

impl<'a> VersionedUpdate<'a> {
    pub async fn new<T>(
        client: &'a mut PGConnection<T>,
        stmts: &'a VersionedUpdateStatements,
        user_id: Uuid,
    ) -> Result<Option<VersionedUpdate<'a>>, IdentityError>
    where
        T: PGRawConnection,
    {
        let transaction = client.transaction().await?;
        let version: i32 = match stmts.get_version.query_opt(&transaction, &user_id).await? {
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

    pub fn client(&self) -> &PGTransaction<'a> {
        &self.transaction
    }

    pub async fn finish(self) -> Result<(), IdentityError> {
        if self
            .stmts
            .update_version
            .execute(&self.transaction, &self.user_id, &self.version)
            .await?
            != 1
        {
            self.transaction.rollback().await?;
            Err(IdentityError::UpdateConflict)
        } else {
            self.transaction.commit().await?;
            Ok(())
        }
    }
}
