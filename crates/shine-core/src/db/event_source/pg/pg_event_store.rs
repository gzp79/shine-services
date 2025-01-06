use crate::{
    db::{
        event_source::{Event, EventStore, EventStoreError, StoredEvent},
        DBError, PGConnectionPool, PGErrorChecks,
    },
    pg_query,
};
use std::{borrow::Cow, marker::PhantomData};
use uuid::Uuid;

pg_query!( CreateStream =>
    in = aggregate:Uuid;
    sql = r#"
        INSERT INTO es_heads_%table% (aggregate_id, version) VALUES ($1, 0)
    "#
);

pg_query!( GetStreamVersion =>
    in = aggregate:Uuid;
    out = version: i32;
    sql = r#"
        SELECT version FROM es_heads_%table% WHERE aggregate_id = $1
    "#
);

pg_query!( UpdateStreamVersion =>
    in = aggregate:Uuid, old_version: i32, new_version: i32;
    sql = r#"
        UPDATE es_heads_%table% SET version = $3 WHERE aggregate_id = $1 AND version = $2
    "#
);

pg_query!( StoreEvent =>
    in = aggregate:Uuid, version: i32, event_type: &str, data: &str;
    sql = r#"
        INSERT INTO es_events_%table% (aggregate_id, version, event_type, data) VALUES ($1, $2, $3, $4::jsonb)
    "#
);

#[derive(Clone)]
pub struct PgEventStore<E>
where
    E: Event,
{
    client: PGConnectionPool,

    table: String,
    create_stream: CreateStream,
    get_version: GetStreamVersion,
    update_version: UpdateStreamVersion,
    store_event: StoreEvent,

    _phantom: PhantomData<E>,
}

impl<E> PgEventStore<E>
where
    E: Event,
{
    pub async fn new(postgres: &PGConnectionPool, table: &str) -> Result<Self, EventStoreError> {
        let client = postgres.get().await.map_err(DBError::PGPoolError)?;

        let table_name_process = |x: &str| Cow::Owned(x.replace("%table%", table));

        Ok(Self {
            client: postgres.clone(),
            table: table.to_string(),
            create_stream: CreateStream::new_with_process(&client, table_name_process)
                .await
                .map_err(DBError::from)?,
            get_version: GetStreamVersion::new_with_process(&client, table_name_process)
                .await
                .map_err(DBError::from)?,
            update_version: UpdateStreamVersion::new_with_process(&client, table_name_process)
                .await
                .map_err(DBError::from)?,
            store_event: StoreEvent::new_with_process(&client, table_name_process)
                .await
                .map_err(DBError::from)?,

            _phantom: PhantomData,
        })
    }
}

impl<E> EventStore for PgEventStore<E>
where
    E: Event,
{
    type Event = E;

    async fn create(&self, aggregate_id: &Uuid) -> Result<(), EventStoreError> {
        let client = self.client.get().await.map_err(DBError::PGPoolError)?;

        if let Err(err) = self.create_stream.execute(&client, &aggregate_id).await {
            if err.is_constraint(
                &format!("es_heads_{}", self.table),
                &format!("es_heads_{}_pkey", self.table),
            ) {
                Err(EventStoreError::Conflict)
            } else {
                Err(DBError::from(err).into())
            }
        } else {
            Ok(())
        }
    }

    async fn store(
        &self,
        aggregate_id: &Uuid,
        expected_version: Option<usize>,
        event: &[Self::Event],
    ) -> Result<usize, EventStoreError> {
        let mut client = self.client.get().await.map_err(DBError::PGPoolError)?;

        let transaction = client.transaction().await.map_err(DBError::from)?;

        let old_version: usize = match self
            .get_version
            .query_opt(&transaction, &aggregate_id)
            .await
            .map_err(DBError::from)?
        {
            Some(version) => version as usize,
            None => return Err(EventStoreError::NotFound),
        };

        if old_version != expected_version.unwrap_or(old_version) {
            transaction.rollback().await.map_err(DBError::from)?;
            return Err(EventStoreError::Conflict);
        }

        let new_version = old_version + event.len();

        for event in event.iter().enumerate() {
            let data = serde_json::to_string(event.1).map_err(EventStoreError::EventSerialization)?;
            self.store_event
                .execute(
                    &transaction,
                    &aggregate_id,
                    &((old_version + event.0) as i32),
                    &event.1.event_type(),
                    &data.as_str(),
                )
                .await
                .map_err(DBError::from)?;
        }

        if self
            .update_version
            .execute(
                &transaction,
                &aggregate_id,
                &(old_version as i32),
                &(new_version as i32),
            )
            .await
            .map_err(DBError::from)?
            != 1
        {
            transaction.rollback().await.map_err(DBError::from)?;
            Err(EventStoreError::Conflict)
        } else {
            transaction.commit().await.map_err(DBError::from)?;
            Ok(new_version)
        }
    }

    async fn get(
        &self,
        aggregate: &Uuid,
        from_version: usize,
        to_version: Option<usize>,
    ) -> Result<Vec<StoredEvent<Self::Event>>, EventStoreError> {
        todo!()
    }
}
