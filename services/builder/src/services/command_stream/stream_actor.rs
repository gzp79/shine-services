use crate::{
    models::{
        messages::{StreamCommand, StreamMessage, StreamSnapshot},
        CommandStreamError,
    },
    repositories::{
        command_inbox::{CommandInboxDb, InboxId},
        command_log::{StoredCommand, StoredSnapshot},
    },
    services::command_stream::{CommandStreamConfig, StreamEgress},
};
use shine_game::replica::{
    decode_command, encode_command, fold_chain_hash, Checkpoint, Command, ReplayState, StreamId,
};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};
use uuid::Uuid;

/// What one pass over the inbox produced.
#[derive(Debug, Default)]
pub struct Ingested {
    /// The commands that were given a version, in inbox order.
    pub commands: Vec<StoredCommand>,
    /// The inbox was not read, or not read to its end, because the optimistic backlog is full. Nothing
    /// is lost: the entries stay in the inbox, but the stream only moves on once persistence catches up.
    pub throttled: bool,
}

/// The inbox and client side of the stream. It takes the commands waiting in the inbox,
/// gives each a version right away and tells the subscribers.
///
/// It starts empty and knows nothing until a reload gives it the history of the log, so a fresh stream
/// and one that lost a race take the same path. The owner holds it back from the inbox and from the
/// subscribers until then.
pub struct StreamActor<S, I, E>
where
    S: ReplayState,
    I: CommandInboxDb,
    E: StreamEgress,
{
    stream_id: StreamId,
    inbox: I,
    egress: E,
    config: CommandStreamConfig,

    snapshot: Option<StoredSnapshot>,
    confirmed: Vec<StoredCommand>,
    confirmed_version: i64,
    confirmed_chain_hash: u64,
    optimistic: Vec<StoredCommand>,
    cursor: InboxId,
    stored_inbox: Option<InboxId>,

    subscribers: HashMap<Uuid, i64>,
    _state: PhantomData<fn() -> S>,
}

impl<S, I, E> StreamActor<S, I, E>
where
    S: ReplayState,
    I: CommandInboxDb,
    E: StreamEgress,
{
    /// An actor that holds nothing of the stream yet: everything it shows comes of the first reload.
    pub fn new(stream_id: StreamId, inbox: I, egress: E, config: CommandStreamConfig) -> Self {
        Self {
            stream_id,
            inbox,
            egress,
            config,
            snapshot: None,
            confirmed: Vec::new(),
            confirmed_version: 0,
            confirmed_chain_hash: 0,
            optimistic: Vec::new(),
            cursor: InboxId::START,
            stored_inbox: None,
            subscribers: HashMap::new(),
            _state: PhantomData,
        }
    }

    pub fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }

    pub fn snapshot(&self) -> Option<&StoredSnapshot> {
        self.snapshot.as_ref()
    }

    pub fn confirmed_version(&self) -> i64 {
        self.confirmed_version
    }

    pub fn confirmed_head(&self) -> (i64, u64) {
        (self.confirmed_version, self.confirmed_chain_hash)
    }

    pub fn confirmed(&self) -> &[StoredCommand] {
        &self.confirmed
    }

    pub fn optimistic(&self) -> &[StoredCommand] {
        &self.optimistic
    }

    pub fn head_version(&self) -> i64 {
        self.optimistic
            .last()
            .map_or(self.confirmed_version, |command| command.version)
    }

    fn head_chain_hash(&self) -> u64 {
        self.optimistic
            .last()
            .map_or(self.confirmed_chain_hash, |command| command.chain_hash)
    }

    pub fn inbox(&self) -> &I {
        &self.inbox
    }

    pub fn cursor(&self) -> &InboxId {
        &self.cursor
    }

    pub fn stored_inbox(&self) -> Option<&InboxId> {
        self.stored_inbox.as_ref()
    }

    pub fn egress(&self) -> &E {
        &self.egress
    }

    pub fn subscribe(&mut self, connection_id: Uuid, sent_through: i64) {
        self.subscribers.insert(connection_id, sent_through);
    }

    pub fn unsubscribe(&mut self, connection_id: Uuid) {
        self.subscribers.remove(&connection_id);
    }

    pub fn subscriber_ids(&self) -> Vec<Uuid> {
        self.subscribers.keys().copied().collect()
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }

    pub fn sent_through(&self, connection_id: Uuid) -> Option<i64> {
        self.subscribers.get(&connection_id).copied()
    }

    //todo: test only, remove
    pub fn record_sent(&mut self, connection_id: Uuid, version: i64) {
        if let Some(sent_through) = self.subscribers.get_mut(&connection_id) {
            *sent_through = (*sent_through).max(version);
        }
    }

    //todo: test only, remove
    pub fn append_optimistic(&mut self, commands: impl IntoIterator<Item = StoredCommand>) {
        self.optimistic.extend(commands);
    }

    /// Takes whatever waits in the inbox and gives each command a version right away
    /// storing them optimistically in the local state.
    pub async fn ingest(&mut self) -> Result<Ingested, CommandStreamError> {
        let room = self.config.max_pending.saturating_sub(self.optimistic.len());
        if room == 0 {
            return Ok(Ingested {
                commands: Vec::new(),
                throttled: true,
            });
        }
        let limit = self.config.poll_limit.min(room);
        let batch = self.inbox.poll(&self.stream_id, &self.cursor, limit).await?;
        let mut version = self.head_version();
        let mut chain_hash = self.head_chain_hash();
        let mut commands = Vec::new();
        for entry in batch.entries {
            if self
                .stored_inbox
                .as_ref()
                .is_some_and(|stored| entry.inbox_id <= *stored)
            {
                continue;
            }
            let command = match decode_command::<S>(&entry.data) {
                Ok(command) if S::is_well_formed(&command) => command,
                _ => {
                    tracing::warn!("[{}] Dropping malformed command {}", self.stream_id, entry.command_id);
                    continue;
                }
            };
            let Ok(data) = encode_command::<S>(&command) else {
                continue;
            };
            let command_hash = command.content_hash();
            version += 1;
            chain_hash = fold_chain_hash(chain_hash, version, command_hash);
            commands.push(StoredCommand {
                version,
                command_id: entry.command_id,
                user_id: entry.user_id,
                data,
                command_hash,
                chain_hash,
                inbox_id: Some(entry.inbox_id.to_string()),
            });
        }
        self.cursor = batch.cursor;
        self.optimistic.extend(commands.iter().cloned());
        let throttled = self.optimistic.len() >= self.config.max_pending;
        Ok(Ingested { commands, throttled })
    }

    /// Update the confirmed state with the canonical batch and snapshot, and reversion the optimistic
    /// commands if necessary. Returns whether the versions the subscribers were told optimistically no
    /// longer hold, so they have to be reset and synced again.
    pub fn accept_canonical(&mut self, canonical: &[StoredCommand], snapshot: Option<StoredSnapshot>) -> bool {
        if let Some(last) = canonical.last() {
            self.confirmed_version = last.version;
            self.confirmed_chain_hash = last.chain_hash;
            self.advance_stored_inbox(canonical);
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

        // Commands keep their order, so what was stored is a prefix of the optimistic ones: the batch was
        // submitted from their front, and the persistence worker reloads instead of confirming anything
        // that does not continue them. Matching by position rather than by id also keeps the same command
        // sent twice as two commands, as it is two inbox entries.
        let stored = canonical
            .iter()
            .zip(self.optimistic.iter())
            .take_while(|(confirmed, optimistic)| confirmed.command_id == optimistic.command_id)
            .count();
        self.optimistic.drain(..stored);
        self.reversion_optimistic()
    }

    /// Puts the optimistic commands back on top of the confirmed history, and reports whether any of
    /// them ended up at a version other than the one the subscribers were already told.
    fn reversion_optimistic(&mut self) -> bool {
        let mut version = self.confirmed_version;
        let mut chain_hash = self.confirmed_chain_hash;
        let mut moved = false;
        for command in &mut self.optimistic {
            version += 1;
            chain_hash = fold_chain_hash(chain_hash, version, command.command_hash);
            moved |= command.version != version;
            command.version = version;
            command.chain_hash = chain_hash;
        }
        moved
    }

    /// The commands of a confirmed batch are in the log, so their inbox entries are taken for good.
    fn advance_stored_inbox(&mut self, canonical: &[StoredCommand]) {
        let stored = canonical
            .iter()
            .rev()
            .find_map(|command| command.inbox_id.as_deref().and_then(InboxId::parse));
        if let Some(stored) = stored {
            if self.cursor < stored {
                self.cursor = stored.clone();
            }
            self.stored_inbox = self.stored_inbox.take().max(Some(stored));
        }
    }

    /// Takes the history of the log, dropping whatever optimistic history this instance held. The first
    /// one is what the actor starts from, so it is also how an empty stream is adopted.
    pub fn accept_reload(
        &mut self,
        snapshot: Option<StoredSnapshot>,
        tail: Vec<StoredCommand>,
        stored_inbox: Option<InboxId>,
    ) {
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
        // The reload read the log, so everything it holds is taken; re-reading the inbox from there on
        // takes the commands of this instance that it did not store again.
        self.cursor = stored_inbox.clone().unwrap_or(InboxId::START);
        self.stored_inbox = stored_inbox;
    }

    /// The state a subscriber starts from: the latest snapshot and the commands the actor shows after it.
    fn sync_message(&self) -> StreamMessage {
        let mut commands: Vec<StreamCommand> = self.confirmed.iter().map(StreamCommand::from).collect();
        commands.extend(self.optimistic.iter().map(StreamCommand::from));

        StreamMessage::Sync {
            stream_id: self.stream_id.clone(),
            snapshot: self.snapshot.as_ref().map(StreamSnapshot::from),
            commands,
        }
    }

    /// Brings one subscriber up to the version the actor shows.
    pub async fn resync(&mut self, connection_id: Uuid) {
        let message = Arc::new(self.sync_message());
        self.send_to_subscriber(connection_id, message, self.head_version())
            .await;
    }

    /// Brings all the subscribers up to the version the actor shows, usually after a reload.
    pub async fn resync_all(&mut self) {
        let reset = Arc::new(StreamMessage::Reset {
            stream_id: self.stream_id.clone(),
        });
        self.broadcast(reset).await;
        let sync = Arc::new(self.sync_message());
        let version = self.head_version();
        for connection_id in self.subscriber_ids() {
            self.send_to_subscriber(connection_id, sync.clone(), version).await;
        }
    }

    /// Sends the commands the subscribers have not seen yet in an optimistic manner.
    pub async fn deliver(&mut self, commands: &[StoredCommand]) {
        let (Some(first), Some(last)) = (commands.first(), commands.last()) else {
            return;
        };
        let (first, last) = (first.version, last.version);
        let all = Arc::new(StreamMessage::Commands {
            stream_id: self.stream_id.clone(),
            commands: commands.iter().map(StreamCommand::from).collect(),
        });
        for connection_id in self.subscriber_ids() {
            let Some(&sent_through) = self.subscribers.get(&connection_id) else {
                continue;
            };
            let message = if sent_through + 1 == first {
                all.clone()
            } else {
                let after: Vec<_> = commands
                    .iter()
                    .filter(|command| command.version > sent_through)
                    .map(StreamCommand::from)
                    .collect();
                if after.is_empty() {
                    continue;
                }
                Arc::new(StreamMessage::Commands {
                    stream_id: self.stream_id.clone(),
                    commands: after,
                })
            };
            self.send_to_subscriber(connection_id, message, last).await;
        }
    }

    /// Tells the subscribers the state hash at a version, so they can compare it with their own.
    pub async fn broadcast_checkpoint(&mut self, checkpoint: Checkpoint) {
        let message = Arc::new(StreamMessage::Checkpoint {
            stream_id: self.stream_id.clone(),
            version: checkpoint.version,
            state_hash: checkpoint.state_hash,
        });
        self.broadcast(message).await;
    }

    /// The stream cannot be served: the subscribers are dropped and have to subscribe again.
    pub async fn give_up(&mut self) {
        let message = Arc::new(StreamMessage::Unsynced {
            stream_id: self.stream_id.clone(),
        });
        self.broadcast(message).await;
        self.subscribers.clear();
    }

    async fn broadcast(&mut self, message: Arc<StreamMessage>) {
        for connection_id in self.subscriber_ids() {
            if !self.egress.send(connection_id, message.clone()).await {
                self.subscribers.remove(&connection_id);
            }
        }
    }

    async fn send_to_subscriber(&mut self, connection_id: Uuid, message: Arc<StreamMessage>, version: i64) {
        if self.egress.send(connection_id, message).await {
            self.subscribers.insert(connection_id, version);
        } else {
            self.subscribers.remove(&connection_id);
        }
    }
}

#[cfg(test)]
#[path = "stream_actor_test.rs"]
mod stream_actor_test;
