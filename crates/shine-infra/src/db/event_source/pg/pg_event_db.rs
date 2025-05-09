use crate::db::{
    event_source::{
        pg::{migration_001, PgEventStoreStatement, PgSnapshotStatement},
        AggregateId, Event, EventDb, EventDbContext, EventNotification, EventStoreError,
    },
    DBError, PGConnectionPool, PGPooledConnection,
};
use serde::Deserialize;
use std::marker::PhantomData;

pub struct PgEventDbContext<'c, E, A>
where
    E: Event,
    A: AggregateId,
{
    pub(in crate::db::event_source::pg) client: PGPooledConnection<'c>,
    pub(in crate::db::event_source::pg) stmts_store: PgEventStoreStatement<E>,
    pub(in crate::db::event_source::pg) stmts_snapshot: PgSnapshotStatement<E>,
    ph: PhantomData<A>,
}

impl<'c, E, A> EventDbContext<'c, E, A> for PgEventDbContext<'c, E, A>
where
    E: Event,
    A: AggregateId,
{
}

pub struct PgEventDb<E, A>
where
    E: Event,
    A: AggregateId,
{
    client: PGConnectionPool,
    stmts_store: PgEventStoreStatement<E>,
    stmts_snapshot: PgSnapshotStatement<E>,
    ph: PhantomData<A>,
}

impl<E, A> PgEventDb<E, A>
where
    E: Event,
    A: AggregateId,
{
    pub async fn new(postgres: &PGConnectionPool) -> Result<Self, EventStoreError> {
        let client = postgres.get().await.map_err(DBError::PGPoolError)?;

        Ok(Self {
            client: postgres.clone(),
            stmts_store: PgEventStoreStatement::new(&client).await?,
            stmts_snapshot: PgSnapshotStatement::new(&client).await?,
            ph: PhantomData,
        })
    }

    pub fn migrations() -> Vec<String> {
        vec![migration_001(E::NAME)]
    }
}

impl<E, A> EventDb<E, A> for PgEventDb<E, A>
where
    E: Event,
    A: AggregateId,
{
    async fn create_context(&self) -> Result<impl EventDbContext<'_, E, A>, EventStoreError> {
        let client = self.client.get().await.map_err(DBError::PGPoolError)?;
        Ok(PgEventDbContext {
            client,
            stmts_store: self.stmts_store.clone(),
            stmts_snapshot: self.stmts_snapshot.clone(),
            ph: PhantomData::<A>,
        })
    }

    async fn listen_to_stream_updates<F>(&self, handler: F) -> Result<(), EventStoreError>
    where
        F: Fn(EventNotification<A>) + Send + Sync + 'static,
    {
        #[derive(Deserialize)]
        struct EventMsg {
            #[serde(rename = "type")]
            ty: String,
            operation: String,
            aggregate_id: String,
            snapshot: Option<String>,
            version: Option<usize>,
        }

        impl EventMsg {
            fn try_into_notification<A>(self) -> Result<EventNotification<A>, String>
            where
                A: AggregateId,
            {
                match (self.ty.as_str(), self.operation.as_str()) {
                    ("stream", "create") => Ok(EventNotification::StreamCreated {
                        aggregate_id: A::from_string(self.aggregate_id),
                        version: self.version.unwrap_or(0),
                    }),
                    ("stream", "update") => Ok(EventNotification::StreamUpdated {
                        aggregate_id: A::from_string(self.aggregate_id),
                        version: self.version.unwrap_or(0),
                    }),
                    ("stream", "delete") => Ok(EventNotification::StreamDeleted {
                        aggregate_id: A::from_string(self.aggregate_id),
                    }),
                    ("snapshot", "create") => Ok(EventNotification::SnapshotCreated {
                        aggregate_id: A::from_string(self.aggregate_id),
                        snapshot: self.snapshot.ok_or("Missing snapshot".to_string())?,
                        version: self.version.unwrap_or(0),
                    }),
                    ("snapshot", "delete") => Ok(EventNotification::SnapshotDeleted {
                        aggregate_id: A::from_string(self.aggregate_id),
                        snapshot: self.snapshot.ok_or("Missing snapshot".to_string())?,
                        version: self.version.unwrap_or(0),
                    }),
                    (ty, op) => Err(format!("Invalid event: [{op},{ty}]")),
                }
            }
        }

        let client = self.client.get().await.map_err(DBError::PGPoolError)?;
        let channel = format!("es_notification_{}", E::NAME);
        log::info!("Listening to event notifications for {}", channel);
        client
            .listen(&channel, move |p| {
                // log::trace!(
                //     "Received event notification on {}: {:?}",
                //     format!("es_notification_{}", E::NAME),
                //     p
                // );
                match serde_json::from_str::<EventMsg>(p)
                    .map_err(|err| format!("Error deserializing event notification: {:#?}", err))
                    .and_then(|msg| msg.try_into_notification())
                {
                    Ok(m) => {
                        handler(m);
                    }
                    Err(e) => log::error!("Unexpected notification: {e}"),
                }
            })
            .await?;
        Ok(())
    }

    async fn unlisten_to_stream_updates(&self) -> Result<(), EventStoreError> {
        let client = self.client.get().await.map_err(DBError::PGPoolError)?;
        client.unlisten(&format!("es_notification_{}", E::NAME)).await?;
        Ok(())
    }
}
