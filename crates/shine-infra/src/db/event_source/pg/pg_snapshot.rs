use crate::{
    db::{
        event_source::{
            pg::PgEventDbContext, Aggregate, AggregateId, Event, EventStore, EventStoreError, Snapshot, SnapshotStore,
        },
        DBError, PGClient, PGErrorChecks,
    },
    pg_query,
};
use postgres_from_row::FromRow;
use std::{borrow::Cow, marker::PhantomData};

pg_query!( StoreSnapshot =>
    in = aggregate: &str, snapshot: &str, start_version: i32, end_version: i32, data: &str;
    sql = r#"
        INSERT INTO es_snapshots_%table% (aggregate_id, snapshot, start_version, version, data) VALUES ($1, $2, $3, $4, $5::jsonb)
    "#
);

#[derive(FromRow)]
struct SnapshotRow {
    start_version: i32,
    version: i32,
    data: String,
}

pg_query!( GetSnapshot =>
    in = aggregate: &str, snapshot: &str, version: Option<i32>;
    out = SnapshotRow;
    sql = r#"
        SELECT start_version, version, data::text FROM es_snapshots_%table% 
            WHERE aggregate_id = $1 AND snapshot = $2 AND ($3 IS NULL OR version <= $3)
            ORDER BY version DESC
            LIMIT 1
    "#
);

pg_query!( PruneSnapshot =>
    in = aggregate: &str, snapshot: &str, version: i32;
    sql = r#"
        DELETE FROM es_snapshots_%table%
            WHERE aggregate_id = $1 AND snapshot = $2 AND version <= $3
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

impl<E, A> SnapshotStore for PgEventDbContext<'_, E, A>
where
    E: Event,
    A: AggregateId,
{
    type Event = E;
    type AggregateId = A;

    async fn get_aggregate<G, D>(
        &mut self,
        aggregate_id: &Self::AggregateId,
        default: D,
    ) -> Result<Snapshot<G>, EventStoreError>
    where
        G: Aggregate<Event = Self::Event, AggregateId = Self::AggregateId>,
        D: FnOnce() -> G + Send + Sync + 'static,
    {
        let mut snapshot = self
            .get_snapshot(aggregate_id, None)
            .await?
            .unwrap_or_else(|| Snapshot::new(aggregate_id.clone(), default));

        snapshot.start_version = snapshot.version;
        // set the current snapshot as the root
        let events = self.get_events(aggregate_id, Some(snapshot.version), None).await?;
        snapshot.apply(events)?;

        Ok(snapshot)
    }

    async fn get_snapshot<G>(
        &mut self,
        aggregate_id: &Self::AggregateId,
        version: Option<usize>,
    ) -> Result<Option<Snapshot<G>>, EventStoreError>
    where
        G: Aggregate<Event = Self::Event, AggregateId = Self::AggregateId>,
    {
        //todo: checking has_stream and getting events are not atomic, it should be improved

        if !self.has_stream(aggregate_id).await? {
            return Err(EventStoreError::AggregateNotFound);
        }

        if let Some(row) = self
            .stmts_snapshot
            .get_snapshot
            .query_opt(
                &self.client,
                &aggregate_id.to_string().as_str(),
                &<G as Aggregate>::NAME,
                &(version.map(|v| v as i32)),
            )
            .await
            .map_err(DBError::from)?
        {
            Ok(Some(Snapshot::from_json(
                aggregate_id.clone(),
                row.start_version as usize,
                row.version as usize,
                &row.data,
            )?))
        } else {
            Ok(None)
        }
    }

    async fn store_snapshot<G>(
        &mut self,
        aggregate_id: &Self::AggregateId,
        start_version: usize,
        version: usize,
        aggregate: &G,
    ) -> Result<(), EventStoreError>
    where
        G: Aggregate<Event = Self::Event, AggregateId = Self::AggregateId>,
    {
        let id = aggregate_id.to_string();
        log::trace!(
            "Storing snapshot for {} with version ({}..{:?}]",
            id,
            start_version,
            version
        );
        let data = serde_json::to_string(aggregate).map_err(EventStoreError::EventSerialization)?;

        match self
            .stmts_snapshot
            .store_snapshot
            .execute(
                &self.client,
                &id.as_str(),
                &G::NAME,
                &(start_version as i32),
                &(version as i32),
                &data.as_str(),
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
                    Err(EventStoreError::Conflict)
                } else if err.is_constraint(
                    &format!("es_snapshots_{}", <E as Event>::NAME),
                    &format!("es_snapshots_{}_aggregate_id_version_fkey", <E as Event>::NAME),
                ) {
                    log::trace!("Missing event for snapshot: {:#?}", err);
                    Err(EventStoreError::EventVersionNotFound(version))
                } else if err.is_constraint(
                    &format!("es_snapshots_{}", <E as Event>::NAME),
                    &format!("es_snapshots_{}_no_branching", <E as Event>::NAME),
                ) {
                    log::trace!("Snapshots shall have no branching: {:#?}", err);
                    Err(EventStoreError::Conflict)
                } else if err.is_raise_exception("Snapshot chain is broken.") {
                    log::trace!("Snapshot parent version does not exist: {:#?}", err);
                    Err(EventStoreError::InvalidSnapshotVersion(start_version, version))
                } else if err.is_constraint(
                    &format!("es_snapshots_{}", <E as Event>::NAME),
                    &format!("es_snapshots_{}_check", <E as Event>::NAME),
                ) {
                    log::trace!("Snapshot versions are invalid: {:#?}", err);
                    Err(EventStoreError::InvalidSnapshotVersion(start_version, version))
                } else {
                    log::info!("Insert snapshot error: {:#?}", err);
                    Err(DBError::from(err).into())
                }
            }
        }
    }

    async fn prune_snapshot<G>(
        &mut self,
        aggregate_id: &Self::AggregateId,
        version: usize,
    ) -> Result<(), EventStoreError>
    where
        G: Aggregate<Event = Self::Event, AggregateId = Self::AggregateId>,
    {
        let id = aggregate_id.to_string();
        log::trace!("Pruning snapshot for {} at version {}", id, version);

        self.stmts_snapshot
            .prune_snapshot
            .execute(&self.client, &id.as_str(), &G::NAME, &(version as i32))
            .await
            .map_err(DBError::from)?;

        Ok(())
    }
}
