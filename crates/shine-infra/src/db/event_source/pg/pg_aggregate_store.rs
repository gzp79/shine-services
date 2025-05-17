use crate::{
    db::{
        event_source::{
            pg::PgEventDbContext, Aggregate, AggregateInfo, AggregateStore, Event, EventSourceError, EventStore,
            StoredAggregate, StreamId,
        },
        DBError, PGClient, PGErrorChecks,
    },
    pg_query,
};
use postgres_from_row::FromRow;
use std::{borrow::Cow, marker::PhantomData};
use uuid::Uuid;

pg_query!( StoreSnapshot =>
    in = stream_id: &str, aggregate_id: &str, start_version: i32, end_version: i32, data: &str, hash: &str;
    sql = r#"
        INSERT INTO es_snapshots_%table% (stream_id, aggregate_id, start_version, version, data, hash)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
    "#
);

#[derive(FromRow)]
struct SnapshotRow {
    stream_token: Uuid,
    start_version: i32,
    version: i32,
    data: String,
    hash: String,
}

pg_query!( GetSnapshot =>
    in = stream_id: &str, aggregate_id: &str, version: Option<i32>;
    out = SnapshotRow;
    sql = r#"
        SELECT h.stream_token, s.start_version, s.version, s.data::text, s.hash 
        FROM es_snapshots_%table% s, es_heads_%table% h
        WHERE s.stream_id = h.stream_id AND s.stream_id = $1 AND s.aggregate_id = $2 AND ($3 IS NULL OR s.version <= $3)
        ORDER BY s.version DESC
        LIMIT 1
    "#
);

#[derive(FromRow)]
struct SnapshotInfoRow {
    stream_token: Uuid,
    start_version: i32,
    version: i32,
    hash: String,
}

pg_query!( ListSnapshots =>
    in = stream_id: &str, aggregate_id: &str;
    out = SnapshotInfoRow;
    sql = r#"
        SELECT h.stream_token, s.start_version, s.version, s.hash 
        FROM es_snapshots_%table% s, es_heads_%table% h
        WHERE s.stream_id = h.stream_id AND s.stream_id = $1 AND s.aggregate_id = $2
        ORDER BY s.version ASC
    "#
);

pg_query!( PruneSnapshot =>
    in = stream_id: &str, aggregate_id: &str, version: i32;
    sql = r#"
        DELETE FROM es_snapshots_%table%
        WHERE stream_id = $1 AND aggregate_id = $2 AND version <= $3
    "#
);

pub struct PgAggregateStoreStatement<E>
where
    E: Event,
{
    store_snapshot: StoreSnapshot,
    get_snapshot: GetSnapshot,
    list_snapshots: ListSnapshots,
    prune_snapshot: PruneSnapshot,

    _ph: PhantomData<fn(&E)>,
}

impl<E> Clone for PgAggregateStoreStatement<E>
where
    E: Event,
{
    fn clone(&self) -> Self {
        Self {
            store_snapshot: self.store_snapshot,
            get_snapshot: self.get_snapshot,
            list_snapshots: self.list_snapshots,
            prune_snapshot: self.prune_snapshot,
            _ph: self._ph,
        }
    }
}

impl<E> PgAggregateStoreStatement<E>
where
    E: Event,
{
    pub async fn new(client: &PGClient) -> Result<Self, EventSourceError> {
        let table_name_process = |x: &str| Cow::Owned(x.replace("%table%", <E as Event>::NAME));

        Ok(Self {
            store_snapshot: StoreSnapshot::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            get_snapshot: GetSnapshot::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            list_snapshots: ListSnapshots::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            prune_snapshot: PruneSnapshot::new_with_process(client, table_name_process)
                .await
                .map_err(DBError::from)?,
            _ph: PhantomData,
        })
    }
}

impl<E, S> AggregateStore for PgEventDbContext<'_, E, S>
where
    E: Event,
    S: StreamId,
{
    type Event = E;
    type StreamId = S;

    async fn store_aggregate<A>(
        &mut self,
        stream_id: &Self::StreamId,
        start_version: usize,
        version: usize,
        aggregate: &A,
        hash: &str,
    ) -> Result<(), EventSourceError>
    where
        A: Aggregate<Event = Self::Event, StreamId = Self::StreamId>,
    {
        let id = stream_id.to_string();
        log::trace!(
            "Storing snapshot {} ({}) with version ({}..{:?}]",
            id,
            hash,
            start_version,
            version
        );
        let data = serde_json::to_string(aggregate).map_err(EventSourceError::EventSerialization)?;

        match self
            .stmts_snapshot
            .store_snapshot
            .execute(
                &self.client,
                &id.as_str(),
                &A::NAME,
                &(start_version as i32),
                &(version as i32),
                &data.as_str(),
                &hash,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.is_constraint(
                    &format!("es_snapshots_{}", <E as Event>::NAME),
                    &format!("es_snapshots_{}_pkey", <E as Event>::NAME),
                ) {
                    log::trace!("Snapshot already exists: {:#?}", err);
                    Err(EventSourceError::Conflict)
                } else if err.is_constraint(
                    &format!("es_snapshots_{}", <E as Event>::NAME),
                    &format!("es_snapshots_{}_stream_id_version_fkey", <E as Event>::NAME),
                ) {
                    log::trace!("Missing event for snapshot: {:#?}", err);
                    Err(EventSourceError::EventVersionNotFound(version))
                } else if err.is_constraint(
                    &format!("es_snapshots_{}", <E as Event>::NAME),
                    &format!("es_snapshots_{}_no_branching", <E as Event>::NAME),
                ) {
                    log::trace!("Snapshots shall have no branching: {:#?}", err);
                    Err(EventSourceError::Conflict)
                } else if err.is_raise_exception("Snapshot chain is broken.") {
                    log::trace!("Snapshot parent version does not exist: {:#?}", err);
                    Err(EventSourceError::InvalidAggregateVersion(start_version, version))
                } else if err.is_constraint(
                    &format!("es_snapshots_{}", <E as Event>::NAME),
                    &format!("es_snapshots_{}_check", <E as Event>::NAME),
                ) {
                    log::trace!("Snapshot versions are invalid: {:#?}", err);
                    Err(EventSourceError::InvalidAggregateVersion(start_version, version))
                } else {
                    log::info!("Insert snapshot error: {:#?}", err);
                    Err(DBError::from(err).into())
                }
            }
        }
    }

    async fn get_aggregate<A>(
        &mut self,
        stream_id: &Self::StreamId,
        version: Option<usize>,
    ) -> Result<Option<StoredAggregate<A>>, EventSourceError>
    where
        A: Aggregate<Event = Self::Event, StreamId = Self::StreamId>,
    {
        //todo: get_stream_version and the next query are not atomic, stream deletion may complete after this call
        if self.get_stream_version(stream_id).await?.is_none() {
            return Err(EventSourceError::StreamNotFound);
        }

        if let Some(row) = self
            .stmts_snapshot
            .get_snapshot
            .query_opt(
                &self.client,
                &stream_id.to_string().as_str(),
                &<A as Aggregate>::NAME,
                &(version.map(|v| v as i32)),
            )
            .await
            .map_err(DBError::from)?
        {
            Ok(Some(StoredAggregate::from_json(
                stream_id.clone(),
                row.stream_token,
                row.start_version as usize,
                row.version as usize,
                &row.data,
                row.hash,
            )?))
        } else {
            Ok(None)
        }
    }

    async fn list_aggregates_by_id(
        &mut self,
        stream_id: &Self::StreamId,
        aggregate_id: &str,
    ) -> Result<Vec<AggregateInfo<S>>, EventSourceError> {
        //todo: get_stream_version and the next query are not atomic, stream deletion may complete after this call
        if self.get_stream_version(stream_id).await?.is_none() {
            return Err(EventSourceError::StreamNotFound);
        }

        let rows = self
            .stmts_snapshot
            .list_snapshots
            .query(&self.client, &stream_id.to_string().as_str(), &aggregate_id)
            .await
            .map_err(DBError::from)?;

        let infos = rows
            .into_iter()
            .map(|row| AggregateInfo {
                stream_id: stream_id.clone(),
                stream_token: row.stream_token,
                start_version: row.start_version as usize,
                version: row.version as usize,
                hash: row.hash,
            })
            .collect();

        Ok(infos)
    }

    async fn prune_aggregate_by_id(
        &mut self,
        stream_id: &Self::StreamId,
        aggregate_id: &str,
        version: usize,
    ) -> Result<(), EventSourceError> {
        let id = stream_id.to_string();
        log::trace!("Pruning snapshot for {} at version {}", id, version);

        self.stmts_snapshot
            .prune_snapshot
            .execute(&self.client, &id.as_str(), &aggregate_id, &(version as i32))
            .await
            .map_err(DBError::from)?;

        Ok(())
    }
}
