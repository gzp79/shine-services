use crate::{
    repositories::{
        command_inbox::CommandInboxDb,
        command_log::{StoredCommand, StoredSnapshot},
    },
    services::command_stream::{PersistJob, PersistResult, PersistenceHandle, PersistenceResults, StreamEgress},
};
use shine_game::replica::StreamId;
use std::collections::HashMap;
use uuid::Uuid;

/// The future fast-path owner for one stream. It is deliberately not wired into `spawn_worker` yet:
/// the current worker keeps serving streams until migration to this actor is done.
pub struct StreamActor<I, E>
where
    I: CommandInboxDb,
    E: StreamEgress,
{
    stream_id: StreamId,
    inbox: I,
    egress: E,
    generation: u64,
    snapshot: Option<StoredSnapshot>,
    confirmed: Vec<StoredCommand>,
    confirmed_version: i64,
    confirmed_chain_hash: u64,
    optimistic: Vec<StoredCommand>,
    subscribers: HashMap<Uuid, i64>,
    persistence: PersistenceHandle,
    persistence_results: PersistenceResults,
    persistence_in_flight: bool,
}

impl<I, E> StreamActor<I, E>
where
    I: CommandInboxDb,
    E: StreamEgress,
{
    pub fn new(
        stream_id: StreamId,
        inbox: I,
        egress: E,
        snapshot: Option<StoredSnapshot>,
        confirmed: Vec<StoredCommand>,
        persistence: PersistenceHandle,
        persistence_results: PersistenceResults,
    ) -> Self {
        let confirmed_version = confirmed.last().map_or_else(
            || snapshot.as_ref().map_or(0, |value| value.version),
            |command| command.version,
        );
        let confirmed_chain_hash = confirmed.last().map_or_else(
            || snapshot.as_ref().map_or(0, |value| value.chain_hash),
            |command| command.chain_hash,
        );
        Self {
            stream_id,
            inbox,
            egress,
            generation: 0,
            snapshot,
            confirmed,
            confirmed_version,
            confirmed_chain_hash,
            optimistic: Vec::new(),
            subscribers: HashMap::new(),
            persistence,
            persistence_results,
            persistence_in_flight: false,
        }
    }

    pub fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }

    pub fn snapshot(&self) -> Option<&StoredSnapshot> {
        self.snapshot.as_ref()
    }

    pub fn confirmed(&self) -> &[StoredCommand] {
        &self.confirmed
    }

    pub fn optimistic(&self) -> &[StoredCommand] {
        &self.optimistic
    }

    pub fn inbox(&self) -> &I {
        &self.inbox
    }

    pub fn egress(&self) -> &E {
        &self.egress
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn subscribe(&mut self, connection_id: Uuid, sent_through: i64) {
        self.subscribers.insert(connection_id, sent_through);
    }

    pub fn unsubscribe(&mut self, connection_id: Uuid) {
        self.subscribers.remove(&connection_id);
    }

    pub fn sent_through(&self, connection_id: Uuid) -> Option<i64> {
        self.subscribers.get(&connection_id).copied()
    }

    pub fn record_sent(&mut self, connection_id: Uuid, version: i64) {
        if let Some(sent_through) = self.subscribers.get_mut(&connection_id) {
            *sent_through = (*sent_through).max(version);
        }
    }

    pub fn append_optimistic(&mut self, commands: impl IntoIterator<Item = StoredCommand>) {
        self.optimistic.extend(commands);
    }

    /// Submits all currently optimistic commands. The actor intentionally permits only one durable
    /// operation at a time, while more optimistic commands may accumulate behind it.
    pub async fn persist_optimistic(&mut self) -> Result<bool, PersistJob> {
        if self.persistence_in_flight || self.optimistic.is_empty() {
            return Ok(false);
        }
        let job = PersistJob {
            generation: self.generation,
            baseline_version: self.confirmed_version,
            baseline_chain_hash: self.confirmed_chain_hash,
            commands: self.optimistic.clone().into(),
        };
        self.persistence.persist(job).await?;
        self.persistence_in_flight = true;
        Ok(true)
    }

    pub async fn recv_persistence(&mut self) -> Option<PersistResult> {
        let result = self.persistence_results.recv().await?;
        self.persistence_in_flight = false;
        Some(result)
    }

    pub fn accept_canonical(
        &mut self,
        generation: u64,
        canonical: &[StoredCommand],
        snapshot: Option<StoredSnapshot>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        if let Some(last) = canonical.last() {
            self.confirmed_version = last.version;
            self.confirmed_chain_hash = last.chain_hash;
        }
        if let Some(snapshot) = snapshot {
            self.confirmed.retain(|command| command.version > snapshot.version);
            self.snapshot = Some(snapshot);
        }
        let snapshot_version = self.snapshot.as_ref().map_or(0, |value| value.version);
        self.confirmed.extend(
            canonical
                .iter()
                .filter(|command| command.version > snapshot_version)
                .cloned(),
        );
        self.optimistic
            .retain(|command| command.version > self.confirmed_version);
        true
    }

    pub fn accept_reload(
        &mut self,
        generation: u64,
        snapshot: Option<StoredSnapshot>,
        tail: Vec<StoredCommand>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        self.generation = self.generation.wrapping_add(1);
        self.confirmed_version = tail.last().map_or_else(
            || snapshot.as_ref().map_or(0, |value| value.version),
            |command| command.version,
        );
        self.confirmed_chain_hash = tail.last().map_or_else(
            || snapshot.as_ref().map_or(0, |value| value.chain_hash),
            |command| command.chain_hash,
        );
        self.snapshot = snapshot;
        self.confirmed = tail;
        self.optimistic.clear();
        self.persistence_in_flight = false;
        true
    }

    /// Invalidates queued results and optimistic history before the owner performs a full reload.
    pub fn begin_reload(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.optimistic.clear();
        self.persistence_in_flight = false;
    }
}

#[cfg(test)]
#[path = "stream_actor_test.rs"]
mod stream_actor_test;
