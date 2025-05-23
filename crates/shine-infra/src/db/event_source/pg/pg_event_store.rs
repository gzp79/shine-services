use crate::{
    db::{
        event_source::{pg::PgEventDbContext, Event, EventSourceError, EventStore, StoredEvent, StreamId},
        DBError, PGClient, PGErrorChecks,
    },
    pg_query,
};
use postgres_from_row::FromRow;
use std::{borrow::Cow, marker::PhantomData};
use tokio_postgres::IsolationLevel;

pg_query!( CreateStream =>
    in = stream_id: &str;
    sql = r#"
        INSERT INTO es_heads_%table% (stream_id, version) VALUES ($1, 0)
    "#
);

pg_query!( DeleteStream =>
    in = stream_id: &str;
    sql = r#"
        DELETE FROM es_heads_%table% WHERE stream_id = $1
    "#
);

pg_query!( GetStreamVersion =>
    in = stream_id: &str;
    out = version: i32;
    sql = r#"
        SELECT version FROM es_heads_%table% WHERE stream_id = $1
    "#
);

pg_query!( UpdateStreamVersion =>
    in = stream_id:&str, old_version: i32, new_version: i32;
    out = version: i32;
    sql = r#"
        UPDATE es_heads_%table% SET version = $3 WHERE stream_id = $1 AND version = $2
        RETURNING version
    "#
);

pg_query!( StoreEvent =>
    in = stream_id: &str, version: i32, event_type: &str, data: &str;
    sql = r#"
        INSERT INTO es_events_%table% (stream_id, version, event_type, data) VALUES ($1, $2, $3, $4::jsonb)
    "#
);

pg_query!( StoreNextEvent =>
    in = stream_id: &str, event_type: &str, data: &str;
    out = version: i32;
    sql = r#"
        WITH upsert_stream AS (
            INSERT INTO es_heads_%table% (stream_id, version)
            VALUES ($1, 1)
            ON CONFLICT (stream_id) DO UPDATE
            SET version = es_heads_test.version + 1
            RETURNING version
        )
        INSERT INTO es_events_%table% (stream_id, version, event_type, data)
        SELECT $1, version, $2, $3::jsonb
        FROM upsert_stream
        RETURNING version;
    "#
);

#[derive(FromRow)]
struct EventRow {
    version: i32,
    data: String,
}

impl EventRow {
    fn try_into_stored_event<E>(self) -> Result<StoredEvent<E>, EventSourceError>
    where
        E: Event,
    {
        Ok(StoredEvent {
            version: self.version as usize,
            event: serde_json::from_str(&self.data).map_err(EventSourceError::EventSerialization)?,
        })
    }
}

pg_query!( GetEvents =>
    in = aggregate: &str, from_version: i32, to_version: i32;
    out = EventRow;
    sql = r#"
        SELECT version, data::text FROM es_events_%table% 
            WHERE stream_id = $1 AND version >= $2 AND version <= $3
            ORDER BY version
    "#
);

pub struct PgEventStoreStatement<E>
where
    E: Event,
{
    create_stream: CreateStream,
    delete_stream: DeleteStream,
    get_version: GetStreamVersion,
    update_version: UpdateStreamVersion,
    store_event: StoreEvent,
    store_next_event: StoreNextEvent,
    get_events: GetEvents,

    _ph: PhantomData<fn(&E)>,
}

impl<E> Clone for PgEventStoreStatement<E>
where
    E: Event,
{
    fn clone(&self) -> Self {
        Self {
            create_stream: self.create_stream,
            delete_stream: self.delete_stream,
            get_version: self.get_version,
            update_version: self.update_version,
            store_event: self.store_event,
            store_next_event: self.store_next_event,
            get_events: self.get_events,
            _ph: self._ph,
        }
    }
}

impl<E> PgEventStoreStatement<E>
where
    E: Event,
{
    pub async fn new(client: &PGClient) -> Result<Self, EventSourceError> {
        let table_name_process = |x: &str| Cow::Owned(x.replace("%table%", <E as Event>::NAME));
        Ok(Self {
            create_stream: CreateStream::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            delete_stream: DeleteStream::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            get_version: GetStreamVersion::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            update_version: UpdateStreamVersion::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            store_event: StoreEvent::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            store_next_event: StoreNextEvent::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            get_events: GetEvents::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,

            _ph: PhantomData,
        })
    }
}

impl<E, S> EventStore for PgEventDbContext<'_, E, S>
where
    E: Event,
    S: StreamId,
{
    type Event = E;
    type StreamId = S;

    async fn create_stream(&mut self, aggregate_id: &Self::StreamId) -> Result<(), EventSourceError> {
        if let Err(err) = self
            .stmts_store
            .create_stream
            .execute(&self.client, &aggregate_id.to_string().as_str())
            .await
        {
            if err.is_constraint(
                &format!("es_heads_{}", <E as Event>::NAME),
                &format!("es_heads_{}_pkey", <E as Event>::NAME),
            ) {
                Err(EventSourceError::Conflict)
            } else {
                Err(DBError::from(err).into())
            }
        } else {
            Ok(())
        }
    }

    async fn get_stream_version(&mut self, aggregate_id: &Self::StreamId) -> Result<Option<usize>, EventSourceError> {
        match self
            .stmts_store
            .get_version
            .query_opt(&self.client, &aggregate_id.to_string().as_str())
            .await
            .map_err(DBError::from)?
        {
            Some(v) => Ok(Some(v as usize)),
            None => Ok(None),
        }
    }

    async fn delete_stream(&mut self, aggregate_id: &Self::StreamId) -> Result<(), EventSourceError> {
        if self
            .stmts_store
            .delete_stream
            .execute(&self.client, &aggregate_id.to_string().as_str())
            .await
            .map_err(DBError::from)?
            != 1
        {
            Err(EventSourceError::StreamNotFound)
        } else {
            Ok(())
        }
    }

    async fn store_events(
        &mut self,
        aggregate_id: &Self::StreamId,
        expected_version: usize,
        event: &[Self::Event],
    ) -> Result<usize, EventSourceError> {
        let transaction = self
            .client
            // read_committed isolation level is used
            //  - only committed changes should be used
            //  - no need for more strict level as the version check ensures failure on concurrent updates
            .transaction(Some(IsolationLevel::ReadCommitted))
            .await
            .map_err(DBError::from)?;

        // Update the header version with a check on the expected version and insert all the events.

        let new_version = expected_version + event.len();

        match self
            .stmts_store
            .update_version
            .query_opt(
                &transaction,
                &aggregate_id.to_string().as_str(),
                &(expected_version as i32),
                &(new_version as i32),
            )
            .await
        {
            Ok(Some(version)) => {
                assert_eq!(version, new_version as i32);
            }
            Ok(None) => {
                transaction.rollback().await.map_err(DBError::from)?;
                // Check of the stream exists and return an error accordingly
                return match self.get_stream_version(aggregate_id).await? {
                    Some(_) => Err(EventSourceError::Conflict),
                    None => Err(EventSourceError::StreamNotFound),
                };
            }
            Err(err) => {
                transaction.rollback().await.map_err(DBError::from)?;
                return Err(DBError::from(err).into());
            }
        }

        for event in event.iter().enumerate() {
            let data = serde_json::to_string(event.1).map_err(EventSourceError::EventSerialization)?;
            if let Err(err) = self
                .stmts_store
                .store_event
                .execute(
                    &transaction,
                    &aggregate_id.to_string().as_str(),
                    &((expected_version + event.0 + 1) as i32),
                    &event.1.event_type(),
                    &data.as_str(),
                )
                .await
            {
                if err.is_constraint(
                    &format!("es_events_{}", <E as Event>::NAME),
                    &format!("es_events_{}_pkey", <E as Event>::NAME),
                ) {
                    transaction.rollback().await.map_err(DBError::from)?;
                    return Err(EventSourceError::Conflict);
                } else {
                    transaction.rollback().await.map_err(DBError::from)?;
                    return Err(DBError::from(err).into());
                }
            }
        }

        transaction.commit().await.map_err(DBError::from)?;
        Ok(new_version)
    }

    async fn unchecked_store_events(
        &mut self,
        aggregate_id: &Self::StreamId,
        event: &[Self::Event],
    ) -> Result<usize, EventSourceError> {
        let mut version = None;
        for event in event.iter() {
            let data = serde_json::to_string(event).map_err(EventSourceError::EventSerialization)?;
            let new_version: i32 = self
                .stmts_store
                .store_next_event
                .query_opt(
                    &self.client,
                    &aggregate_id.to_string().as_str(),
                    &event.event_type(),
                    &data.as_str(),
                )
                .await
                .map_err(DBError::from)?
                .expect("Failed to store event without a DB error");
            version = Some(new_version as usize);
        }

        if let Some(version) = version {
            Ok(version)
        } else {
            log::warn!("Performance warning: store_event called without any events");
            match self
                .stmts_store
                .get_version
                .query_opt(&self.client, &aggregate_id.to_string().as_str())
                .await
                .map_err(DBError::from)?
            {
                Some(version) => Ok(version as usize),
                None => Ok(0),
            }
        }
    }

    async fn get_events(
        &mut self,
        aggregate_id: &Self::StreamId,
        from_version: Option<usize>,
        to_version: Option<usize>,
    ) -> Result<Vec<StoredEvent<Self::Event>>, EventSourceError> {
        let fv = from_version.map(|v| v as i32).unwrap_or(0);
        let tv = to_version.map(|v| v as i32).unwrap_or(i32::MAX);

        //todo: checking has_stream and getting events are not atomic, it should be improved

        if self.get_stream_version(aggregate_id).await?.is_none() {
            return Err(EventSourceError::StreamNotFound);
        }

        let events = self
            .stmts_store
            .get_events
            .query(&self.client, &aggregate_id.to_string().as_str(), &fv, &tv)
            .await
            .map_err(DBError::from)?
            .into_iter()
            .map(|row| row.try_into_stored_event())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }
}
