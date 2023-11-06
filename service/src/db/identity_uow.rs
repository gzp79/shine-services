use crate::db::{IdentityBuildError, IdentityError};
use shine_service::{
    pg_query,
    service::{PGClientOrTransaction, PGPooledConnection, PGTransaction},
};
use uuid::Uuid;

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

pub struct IdentityVersionStatements {
    get_version: GetDataVersion,
    update_version: UpdateDataVersion,
}

impl IdentityVersionStatements {
    pub async fn new(client: &PGPooledConnection<'_>) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            get_version: GetDataVersion::new(client).await?,
            update_version: UpdateDataVersion::new(client).await?,
        })
    }
}

pub struct IdentityUOW<'a> {
    transaction: PGTransaction<'a>,
    stmts: &'a IdentityVersionStatements,
    user_id: Uuid,
    version: i32,
}

impl<'a> IdentityUOW<'a> {
    pub async fn new(
        transaction: PGTransaction<'a>,
        stmts: &'a IdentityVersionStatements,
        user_id: Uuid,
    ) -> Result<Option<IdentityUOW<'a>>, IdentityError> {
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

impl<'a> From<&'a IdentityUOW<'a>> for PGClientOrTransaction<'a> {
    fn from(value: &'a IdentityUOW<'a>) -> Self {
        PGClientOrTransaction::Transaction(&value.transaction)
    }
}
