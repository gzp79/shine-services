use crate::db::{
    event_source::{
        pg::{PgEventStoreStatement, PgSnapshotStatement},
        Event, EventDb, EventDbContext, EventStoreError,
    },
    DBError, PGConnectionPool, PGPooledConnection,
};

pub struct PgEventDbContext<'c, E>
where
    E: Event,
{
    pub(in crate::db::event_source::pg) client: PGPooledConnection<'c>,
    pub(in crate::db::event_source::pg) stmts_store: PgEventStoreStatement<E>,
    pub(in crate::db::event_source::pg) stmts_snapshot: PgSnapshotStatement<E>,
}

impl<'c, E> EventDbContext<'c, E> for PgEventDbContext<'c, E> where E: Event {}

pub struct PgEventDb<E>
where
    E: Event,
{
    client: PGConnectionPool,
    stmts_store: PgEventStoreStatement<E>,
    stmts_snapshot: PgSnapshotStatement<E>,
}

impl<E> PgEventDb<E>
where
    E: Event,
{
    pub async fn new(postgres: &PGConnectionPool) -> Result<Self, EventStoreError> {
        let client = postgres.get().await.map_err(DBError::PGPoolError)?;

        Ok(Self {
            client: postgres.clone(),
            stmts_store: PgEventStoreStatement::new(&client).await?,
            stmts_snapshot: PgSnapshotStatement::new(&client).await?,
        })
    }
}

impl<E> EventDb<E> for PgEventDb<E>
where
    E: Event,
{
    async fn create_context(&self) -> Result<impl EventDbContext<'_, E>, EventStoreError> {
        let client = self.client.get().await.map_err(DBError::PGPoolError)?;
        Ok(PgEventDbContext {
            client,
            stmts_store: self.stmts_store.clone(),
            stmts_snapshot: self.stmts_snapshot.clone(),
        })
    }
}
