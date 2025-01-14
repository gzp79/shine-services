use crate::{
    db::{
        event_source::{pg::PgEventDbContext, Aggregate, Event, EventStore, EventStoreError, Snapshot, SnapshotStore},
        DBError, PGClient, PGErrorChecks,
    },
    pg_query,
};
use postgres_from_row::FromRow;
use std::{borrow::Cow, marker::PhantomData};
use uuid::Uuid;

pg_query!( StoreSnapshot =>
    in = aggregate: Uuid, snapshot: &str, version: i32, data: &str;
    sql = r#"
        INSERT INTO es_snapshots_%table% (aggregate_id, snapshot, version, data) VALUES ($1, $2, $3, $4::jsonb)
    "#
);

#[derive(FromRow)]
struct SnapshotRow {
    version: i32,
    data: String,
}

pg_query!( GetSnapshot =>
    in = aggregate: Uuid, snapshot: &str;
    out = SnapshotRow;
    sql = r#"
        SELECT version, data::text FROM es_snapshots_%table% 
            WHERE aggregate_id = $1 AND snapshot = $2
            ORDER BY version
            LIMIT 1
    "#
);

pg_query!( PruneSnapshot =>
    in = aggregate: Uuid, snapshot: &str, version: i32;
    sql = r#"
        DELETE FROM es_snapshots_%table%
            WHERE aggregate_id = $1 AND snapshot = $2 AND version < $3
    "#
);

pub struct PgSnapshotStatement<E>
where
    E: Event,
{
    store_snapshot: StoreSnapshot,
    get_snapshot: GetSnapshot,
    prune_snapshot: PruneSnapshot,

    _ph: PhantomData<fn(&E)>,
}

impl<E> Clone for PgSnapshotStatement<E>
where
    E: Event,
{
    fn clone(&self) -> Self {
        Self {
            store_snapshot: self.store_snapshot,
            get_snapshot: self.get_snapshot,
            prune_snapshot: self.prune_snapshot,
            _ph: self._ph,
        }
    }
}

impl<E> PgSnapshotStatement<E>
where
    E: Event,
{
    pub async fn new(client: &PGClient) -> Result<Self, EventStoreError> {
        let table_name_process = |x: &str| Cow::Owned(x.replace("%table%", <E as Event>::NAME));

        Ok(Self {
            store_snapshot: StoreSnapshot::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            get_snapshot: GetSnapshot::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            prune_snapshot: PruneSnapshot::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,

            _ph: PhantomData,
        })
    }
}

impl<'c, E> SnapshotStore for PgEventDbContext<'c, E>
where
    E: Event,
{
    type Event = E;

    async fn get_aggregate<A>(&mut self, aggregate_id: &Uuid) -> Result<Option<Snapshot<A>>, EventStoreError>
    where
        A: Aggregate<Event = Self::Event>,
    {
        let mut snapshot = self
            .get_snapshot(aggregate_id)
            .await?
            .unwrap_or(Snapshot::new(*aggregate_id));

        let events = self.get_events(aggregate_id, Some(snapshot.version()), None).await?;
        snapshot.apply(&events)?;

        Ok(Some(snapshot))
    }

    async fn get_snapshot<A>(&mut self, aggregate_id: &Uuid) -> Result<Option<Snapshot<A>>, EventStoreError>
    where
        A: Aggregate<Event = Self::Event>,
    {
        //todo: checking has_stream and getting events are not atomic, it should be improved

        if !self.has_stream(aggregate_id).await? {
            return Err(EventStoreError::NotFound);
        }

        if let Some(row) = self
            .stmts_snapshot
            .get_snapshot
            .query_opt(&self.client, aggregate_id, &<A as Aggregate>::NAME)
            .await
            .map_err(DBError::from)?
        {
            Ok(Some(Snapshot::new_from_data(
                *aggregate_id,
                row.version as usize,
                &row.data,
            )?))
        } else {
            Ok(None)
        }
    }

    async fn store_snapshot<A>(&mut self, snapshot: &Snapshot<A>) -> Result<(), EventStoreError>
    where
        A: Aggregate<Event = Self::Event>,
    {
        let data = serde_json::to_string(snapshot.aggregate()).map_err(EventStoreError::EventSerialization)?;

        if let Err(err) = self
            .stmts_snapshot
            .store_snapshot
            .execute(
                &self.client,
                snapshot.id(),
                &A::NAME,
                &(snapshot.version() as i32),
                &data.as_str(),
            )
            .await
        {
            if err.is_constraint(
                &format!("es_snapshots_{}", <E as Event>::NAME),
                &format!("es_snapshots_{}_pkey", <E as Event>::NAME),
            ) {
                Err(EventStoreError::Conflict)
            } else {
                Err(DBError::from(err).into())
            }
        } else {
            if let Err(err) = self
                .stmts_snapshot
                .prune_snapshot
                .execute(&self.client, snapshot.id(), &A::NAME, &(snapshot.version() as i32))
                .await
            {
                log::error!("Failed to prune snapshots: {:#?}", err);
            }

            Ok(())
        }
    }
}
