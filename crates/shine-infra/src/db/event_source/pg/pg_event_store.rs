use crate::{
    db::{
        event_source::{pg::PgEventDbContext, AggregateId, Event, EventStore, EventStoreError, StoredEvent},
        DBError, PGClient, PGErrorChecks,
    },
    pg_query,
};
use postgres_from_row::FromRow;
use std::{borrow::Cow, marker::PhantomData};
use tokio_postgres::IsolationLevel;

pg_query!( CreateStream =>
    in = aggregate: &str;
    sql = r#"
        INSERT INTO es_heads_%table% (aggregate_id, version) VALUES ($1, 0)
    "#
);

pg_query!( DeleteStream =>
    in = aggregate: &str;
    sql = r#"
        DELETE FROM es_heads_%table% WHERE aggregate_id = $1
    "#
);

pg_query!( GetStreamVersion =>
    in = aggregate: &str;
    out = version: i32;
    sql = r#"
        SELECT version FROM es_heads_%table% WHERE aggregate_id = $1
    "#
);

pg_query!( UpdateStreamVersion =>
    in = aggregate:&str, old_version: i32, new_version: i32;
    sql = r#"
        UPDATE es_heads_%table% SET version = $3 WHERE aggregate_id = $1 AND version = $2
    "#
);

pg_query!( StoreEvent =>
    in = aggregate: &str, version: i32, event_type: &str, data: &str;
    sql = r#"
        INSERT INTO es_events_%table% (aggregate_id, version, event_type, data) VALUES ($1, $2, $3, $4::jsonb)
    "#
);

pg_query!( StoreNextEvent =>
    in = aggregate: &str, event_type: &str, data: &str;
    out = version: i32;
    sql = r#"
        WITH updated_stream AS (
            UPDATE es_heads_%table%
            SET version = version + 1
            WHERE aggregate_id = $1
            RETURNING version
        ), created_stream AS (
            INSERT INTO es_heads_%table% (aggregate_id, version)
            SELECT $1, 1
            WHERE NOT EXISTS (SELECT 1 FROM updated_stream)
            RETURNING version
        ), final_version AS (
            SELECT version FROM updated_stream
            UNION ALL
            SELECT version FROM created_stream
        )
        INSERT INTO es_events_%table% (aggregate_id, version, event_type, data)
        SELECT $1, version, $2, $3::jsonb
        FROM final_version
        RETURNING version;
    "#
);

#[derive(FromRow)]
struct EventRow {
    version: i32,
    data: String,
}

impl EventRow {
    fn try_into_stored_event<E>(self) -> Result<StoredEvent<E>, EventStoreError>
    where
        E: Event,
    {
        Ok(StoredEvent {
            version: self.version as usize,
            event: serde_json::from_str(&self.data).map_err(EventStoreError::EventSerialization)?,
        })
    }
}

pg_query!( GetEvent =>
    in = aggregate: &str, from_version: i32, to_version: i32;
    out = EventRow;
    sql = r#"
        SELECT version, data::text FROM es_events_%table% 
            WHERE aggregate_id = $1 AND version >= $2 AND version <= $3
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
    get_event: GetEvent,

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
            get_event: self.get_event,
            _ph: self._ph,
        }
    }
}

impl<E> PgEventStoreStatement<E>
where
    E: Event,
{
    pub async fn new(client: &PGClient) -> Result<Self, EventStoreError> {
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
            get_event: GetEvent::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,

            _ph: PhantomData,
        })
    }
}

impl<E, A> EventStore for PgEventDbContext<'_, E, A>
where
    E: Event,
    A: AggregateId,
{
    type Event = E;
    type AggregateId = A;

    async fn create_stream(&mut self, aggregate_id: &Self::AggregateId) -> Result<(), EventStoreError> {
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
                Err(EventStoreError::Conflict)
            } else {
                Err(DBError::from(err).into())
            }
        } else {
            Ok(())
        }
    }

    async fn has_stream(&mut self, aggregate_id: &Self::AggregateId) -> Result<bool, EventStoreError> {
        match self
            .stmts_store
            .get_version
            .query_opt(&self.client, &aggregate_id.to_string().as_str())
            .await
            .map_err(DBError::from)?
        {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    async fn delete_stream(&mut self, aggregate_id: &Self::AggregateId) -> Result<(), EventStoreError> {
        if self
            .stmts_store
            .delete_stream
            .execute(&self.client, &aggregate_id.to_string().as_str())
            .await
            .map_err(DBError::from)?
            != 1
        {
            Err(EventStoreError::AggregateNotFound)
        } else {
            Ok(())
        }
    }

    async fn store_events(
        &mut self,
        aggregate_id: &Self::AggregateId,
        expected_version: usize,
        event: &[Self::Event],
    ) -> Result<usize, EventStoreError> {
        let transaction = self
            .client
            .transaction(Some(IsolationLevel::RepeatableRead))
            .await
            .map_err(DBError::from)?;

        let old_version: usize = match self
            .stmts_store
            .get_version
            .query_opt(&transaction, &aggregate_id.to_string().as_str())
            .await
            .map_err(DBError::from)?
        {
            Some(version) => version as usize,
            None => return Err(EventStoreError::AggregateNotFound),
        };

        if old_version != expected_version {
            transaction.rollback().await.map_err(DBError::from)?;
            return Err(EventStoreError::Conflict);
        }

        let new_version = old_version + event.len();

        for event in event.iter().enumerate() {
            let data = serde_json::to_string(event.1).map_err(EventStoreError::EventSerialization)?;
            self.stmts_store
                .store_event
                .execute(
                    &transaction,
                    &aggregate_id.to_string().as_str(),
                    &((old_version + event.0 + 1) as i32),
                    &event.1.event_type(),
                    &data.as_str(),
                )
                .await
                .map_err(DBError::from)?;
        }

        if self
            .stmts_store
            .update_version
            .execute(
                &transaction,
                &aggregate_id.to_string().as_str(),
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

    async fn unchecked_store_events(
        &mut self,
        aggregate_id: &Self::AggregateId,
        event: &[Self::Event],
    ) -> Result<usize, EventStoreError> {
        let mut version = None;
        for event in event.iter() {
            let data = serde_json::to_string(event).map_err(EventStoreError::EventSerialization)?;
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
        aggregate_id: &Self::AggregateId,
        from_version: Option<usize>,
        to_version: Option<usize>,
    ) -> Result<Vec<StoredEvent<Self::Event>>, EventStoreError> {
        let fv = from_version.map(|v| v as i32).unwrap_or(0);
        let tv = to_version.map(|v| v as i32).unwrap_or(i32::MAX);

        //todo: checking has_stream and getting events are not atomic, it should be improved

        if !self.has_stream(aggregate_id).await? {
            return Err(EventStoreError::AggregateNotFound);
        }

        let events = self
            .stmts_store
            .get_event
            .query(&self.client, &aggregate_id.to_string().as_str(), &fv, &tv)
            .await
            .map_err(DBError::from)?
            .into_iter()
            .map(|row| row.try_into_stored_event())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }
}
