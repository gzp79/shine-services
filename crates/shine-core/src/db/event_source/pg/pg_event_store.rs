use crate::{
    db::{
        event_source::{Event, EventStore, EventStoreError, StoredEvent},
        DBError, PGConnectionPool,
    },
    pg_query,
};
use std::marker::PhantomData;
use uuid::Uuid;

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

#[derive(Clone)]
pub struct PgEventStore<E>
where
    E: Event,
{
    client: PGConnectionPool,

    get_version: GetStreamVersion,
    update_version: UpdateStreamVersion,
    _phantom: PhantomData<E>,
}

impl<E> PgEventStore<E>
where
    E: Event,
{
    pub async fn new(client: &PGConnectionPool, table: &str) -> Result<Self, EventStoreError> {
        let (get_version, update_version) = {
            let client = client.get().await.map_err(DBError::PGPoolError)?;
            (
                GetStreamVersion::new_with_process(&client, |x| x.replace("%table%", table).into())
                    .await
                    .map_err(DBError::from)?,
                UpdateStreamVersion::new_with_process(&client, |x| x.replace("%table%", table).into())
                    .await
                    .map_err(DBError::from)?,
            )
        };

        Ok(Self {
            client: client.clone(),
            get_version,
            update_version,
            _phantom: PhantomData,
        })
    }
}

impl<E> EventStore for PgEventStore<E>
where
    E: Event,
{
    type Event = E;

    async fn store(
        &self,
        aggregate_id: Uuid,
        expected_version: usize,
        event: &[Self::Event],
    ) -> Result<usize, EventStoreError> {
        let mut client = self.client.get().await.map_err(DBError::PGPoolError)?;

        let transaction = client.transaction().await.map_err(DBError::from)?;

        let version: usize = match self
            .get_version
            .query_opt(&transaction, &aggregate_id)
            .await
            .map_err(DBError::from)?
        {
            Some(version) => version as usize,
            None => return Err(EventStoreError::NotFound),
        };

        if version != expected_version {
            transaction.rollback().await.map_err(DBError::from)?;
            return Err(EventStoreError::Conflict);
        }

        let new_version = version + event.len();

        //self.store_events

        if self
            .update_version
            .execute(
                &transaction,
                &aggregate_id,
                &(expected_version as i32),
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
        aggregate: Uuid,
        from_version: usize,
        to_version: usize,
    ) -> Result<Vec<StoredEvent<Self::Event>>, EventStoreError> {
        todo!()
    }
}
