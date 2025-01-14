use serde::Deserialize;
use uuid::Uuid;

use crate::db::{
    event_source::{
        pg::{PgEventStoreStatement, PgSnapshotStatement},
        Event, EventDb, EventDbContext, EventNotification, EventStoreError,
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

    async fn listen_to_events<F>(&self, handler: F) -> Result<(), EventStoreError>
    where
        F: Fn(EventNotification) -> () + Send + Sync + 'static,
    {
        #[derive(Deserialize)]
        struct EventMsg {
            aggregate_id: Uuid,
            version: usize,
        }

        let client = self.client.get().await.map_err(DBError::PGPoolError)?;
        client
            .listen(
                &format!("es_notification_{}", E::NAME),
                move |p| match serde_json::from_str::<EventMsg>(p).map(|msg| EventNotification {
                    aggregate_id: msg.aggregate_id,
                    version: msg.version,
                }) {
                    Ok(m) => {
                        handler(m);
                    }
                    Err(e) => log::error!("Error deserializing event notification: {:#?}", e),
                },
            )
            .await?;
        Ok(())
    }

    async fn unlisten_to_events(&self) -> Result<(), EventStoreError> {
        let client = self.client.get().await.map_err(DBError::PGPoolError)?;
        client.unlisten(&format!("es_notification_{}", E::NAME)).await?;
        Ok(())
    }
}
