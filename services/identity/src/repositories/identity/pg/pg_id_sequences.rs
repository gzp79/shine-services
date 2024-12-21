use crate::repositories::{
    identity::{IdSequences, IdentityBuildError, IdentityError},
    DBError,
};
use shine_core::{pg_query, service::PGClient};

use super::PgIdentityDbContext;

pg_query!( GetNextId =>
    in = ;
    out = id: i64;
    sql = r#"
        SELECT nextval('user_id_counter') as id
    "#
);

#[derive(Clone)]
pub struct PgIdSequencesStatements {
    stmt_next_id: GetNextId,
}

impl PgIdSequencesStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            stmt_next_id: GetNextId::new(client).await.map_err(DBError::from)?,
        })
    }
}

impl<'a> IdSequences for PgIdentityDbContext<'a> {
    async fn get_next_id(&mut self) -> Result<u64, IdentityError> {
        let id = self
            .stmts_id_sequences
            .stmt_next_id
            .query_one(&self.client)
            .await
            .map_err(DBError::from)?;
        Ok(id as u64)
    }
}
