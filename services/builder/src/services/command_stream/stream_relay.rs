use crate::{
    models::{
        messages::{StreamCommand, StreamMessage, StreamSnapshot},
        CommandStreamError,
    },
    repositories::{
        command_inbox::{CommandInboxDb, InboxId},
        command_log::{StoredCommand, StoredSnapshot},
    },
    services::command_stream::{stream_persistence::PersistenceWork, PersistResult, StreamConfig, StreamEgress},
};
use shine_game::replica::{decode_command, fold_chain_hash, Checkpoint, Command, ReplayState, StreamId};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};
use uuid::Uuid;

/// The inbox and client side of the stream. It takes the commands waiting in the inbox,
/// gives each a version right away and tells the subscribers.
///
/// It starts empty and knows nothing until a reload gives it the history of the log, so a fresh stream
/// and one that lost a race take the same path. The owner holds it back from the inbox and from the
/// subscribers until then.
pub struct StreamRelay<S, I, E>
where
    S: ReplayState,
    I: CommandInboxDb,
    E: StreamEgress,
{
    stream_id: StreamId,
    inbox: I,
    egress: E,
    config: StreamConfig,

    snapshot: Option<StoredSnapshot>,
    confirmed: Vec<StoredCommand>,
    confirmed_version: i64,
    confirmed_chain_hash: u64,
    optimistic: Vec<StoredCommand>,
    /// Where the next poll of the inbox resumes: the last position that has been read.
    read_index: InboxId,
    stored_inbox: Option<InboxId>,

    subscribers: HashMap<Uuid, Option<i64>>,
    /// The last checkpoint the persistence confirmed that the subscribers were not told yet.
    pending_checkpoint: Option<Checkpoint>,
    _state: PhantomData<fn() -> S>,
}

impl<S, I, E> StreamRelay<S, I, E>
where
    S: ReplayState,
    I: CommandInboxDb,
    E: StreamEgress,
{
    /// A relay that holds no stream history yet: everything it shows comes from the first reload.
    pub fn new(stream_id: StreamId, inbox: I, egress: E, config: StreamConfig) -> Self {
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
            read_index: InboxId::START,
            stored_inbox: None,
            subscribers: HashMap::new(),
            pending_checkpoint: None,
            _state: PhantomData,
        }
    }

    pub fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }

    pub fn egress(&self) -> &E {
        &self.egress
    }

    pub fn subscribe(&mut self, connection_id: Uuid) {
        self.subscribers.insert(connection_id, None);
    }

    pub fn unsubscribe(&mut self, connection_id: Uuid) {
        self.subscribers.remove(&connection_id);
    }

    pub fn has_subscribers(&self) -> bool {
        !self.subscribers.is_empty()
    }

    /// The work that adopts the history of the log: the persistence checks the confirmed head and reloads
    /// the history if it differs.
    pub fn reconcile_work(&self) -> PersistenceWork {
        PersistenceWork::Reconcile {
            version: self.confirmed_version,
            chain_hash: self.confirmed_chain_hash,
        }
    }

    /// The work that makes the optimistic commands durable, if there are any.
    pub fn work_to_persist(&self) -> Option<PersistenceWork> {
        if self.optimistic.is_empty() {
            return None;
        }
        Some(PersistenceWork::Persist {
            baseline_version: self.confirmed_version,
            baseline_chain_hash: self.confirmed_chain_hash,
            commands: self.optimistic.clone().into(),
        })
    }

    /// Takes the answer of the persistence. Only bookkeeping: nothing is sent to the subscribers, they are
    /// told by the next delivery. An error means the stream cannot go on.
    pub fn accept(&mut self, result: PersistResult) -> Result<(), CommandStreamError> {
        match result {
            PersistResult::Current => {}
            PersistResult::Advanced {
                canonical,
                snapshot,
                checkpoint,
            } => {
                self.accept_canonical(&canonical, snapshot);
                // Only the latest one matters, an older one that was not sent yet is obsolete.
                self.pending_checkpoint = checkpoint.or(self.pending_checkpoint.take());
            }
            PersistResult::Reloaded {
                snapshot,
                tail,
                stored_inbox,
                reason,
            } => {
                tracing::info!("[{}] The stream is reloaded: {reason}", self.stream_id);
                self.accept_reload(snapshot, tail.to_vec(), stored_inbox);
            }
            PersistResult::Failed { error } => return Err(error),
        }
        Ok(())
    }

    /// Reads a page of the inbox, gives each command in it a version right away and stores them
    /// optimistically in the local state. Returns whether the inbox may hold more than the page, and
    /// there is room to read it.
    ///
    /// While the optimistic tail is full the inbox is not read: the entries stay there until persistence
    /// frees room, and the answer of the persistence is what brings the next read.
    pub async fn ingest(&mut self) -> Result<bool, CommandStreamError> {
        let room = self.config.relay_max_pending.saturating_sub(self.optimistic.len());
        if room == 0 {
            return Ok(false);
        }
        let limit = self.config.inbox_poll_limit.min(room);
        let batch = self.inbox.poll(&self.stream_id, &self.read_index, limit).await?;
        let (mut version, mut chain_hash) = self.head();
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
            let command_hash = command.content_hash();
            version += 1;
            chain_hash = fold_chain_hash(chain_hash, version, command_hash);
            self.optimistic.push(StoredCommand {
                version,
                command_id: entry.command_id,
                user_id: entry.user_id,
                data: entry.data,
                command_hash,
                chain_hash,
                inbox_id: entry.inbox_id,
            });
        }
        // A page that did not move the position would be read again and again.
        let advanced = batch.cursor != self.read_index;
        self.read_index = batch.cursor;
        Ok(batch.has_more && advanced && self.optimistic.len() < self.config.relay_max_pending)
    }

    /// Brings every subscriber up to the version the relay shows: the ones that need a sync get one, the
    /// others get the commands they have not seen yet. It is the only place that talks to the subscribers.
    pub async fn deliver(&mut self) {
        let (head, _) = self.head();
        let mut sync: Option<Arc<StreamMessage>> = None;
        let mut follow_ups: HashMap<i64, Option<Arc<StreamMessage>>> = HashMap::new();
        for connection_id in self.subscribers.keys().copied().collect::<Vec<_>>() {
            let Some(&cursor) = self.subscribers.get(&connection_id) else {
                continue;
            };
            let follow_up = match cursor {
                Some(sent_through) if sent_through >= head => continue,
                Some(sent_through) => follow_ups
                    .entry(sent_through)
                    .or_insert_with(|| self.follow_up(sent_through))
                    .clone(),
                None => None,
            };
            let message =
                follow_up.unwrap_or_else(|| sync.get_or_insert_with(|| Arc::new(self.sync_message())).clone());
            self.send_to_subscriber(connection_id, message, head).await;
        }

        // Every subscriber has the commands up to the checkpoint by now, so they can compare their state.
        if let Some(checkpoint) = self.pending_checkpoint.take() {
            let message = Arc::new(StreamMessage::Checkpoint {
                stream_id: self.stream_id.clone(),
                version: checkpoint.version,
                state_hash: checkpoint.state_hash,
            });
            self.broadcast(message).await;
        }
    }

    /// The stream cannot be served: the subscribers are dropped and have to subscribe again.
    pub async fn give_up(&mut self) {
        let message = Arc::new(StreamMessage::Unsynced {
            stream_id: self.stream_id.clone(),
        });
        self.broadcast(message).await;
        self.subscribers.clear();
    }

    /// The version and chain hash of the last command the relay shows, optimistic ones included.
    fn head(&self) -> (i64, u64) {
        self.optimistic
            .last()
            .map_or((self.confirmed_version, self.confirmed_chain_hash), |command| {
                (command.version, command.chain_hash)
            })
    }

    /// Update the confirmed state with the canonical batch and snapshot, and reversion the optimistic
    /// commands if necessary. If the versions the subscribers were told optimistically no longer hold,
    /// they are marked to be synced again.
    fn accept_canonical(&mut self, canonical: &[StoredCommand], snapshot: Option<StoredSnapshot>) {
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
        if self.reversion_optimistic() {
            self.reset_subscribers();
        }
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
        let stored = canonical.iter().map(|command| command.inbox_id).max();
        if let Some(stored) = stored {
            if self.read_index < stored {
                self.read_index = stored;
            }
            self.stored_inbox = self.stored_inbox.take().max(Some(stored));
        }
    }

    /// Takes the history of the log, dropping whatever optimistic history this instance held. The first
    /// one is what the relay starts from, so it is also how an empty stream is adopted.
    fn accept_reload(
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
        self.pending_checkpoint = None;
        self.read_index = stored_inbox.unwrap_or(InboxId::START);
        self.stored_inbox = stored_inbox;
        self.reset_subscribers();
    }

    /// The state a subscriber starts from: the latest snapshot and the commands the relay shows after it.
    fn sync_message(&self) -> StreamMessage {
        let mut commands: Vec<StreamCommand> = self.confirmed.iter().map(StreamCommand::from).collect();
        commands.extend(self.optimistic.iter().map(StreamCommand::from));

        StreamMessage::Sync {
            stream_id: self.stream_id.clone(),
            snapshot: self.snapshot.as_ref().map(StreamSnapshot::from),
            commands,
        }
    }

    /// The commands after the version, as the subscribers that have it all but these need them. `None`
    /// when the relay does not hold them contiguously any more, so the subscriber needs a sync.
    fn follow_up(&self, after: i64) -> Option<Arc<StreamMessage>> {
        let commands: Vec<&StoredCommand> = self
            .confirmed
            .iter()
            .chain(&self.optimistic)
            .filter(|command| command.version > after)
            .collect();
        if commands.first()?.version != after + 1 {
            return None;
        }
        Some(Arc::new(StreamMessage::Commands {
            stream_id: self.stream_id.clone(),
            commands: commands.into_iter().map(StreamCommand::from).collect(),
        }))
    }

    /// Resets all subscriber cursors, indicating that they need to be synced again.
    fn reset_subscribers(&mut self) {
        for cursor in self.subscribers.values_mut() {
            *cursor = None;
        }
    }

    async fn broadcast(&mut self, message: Arc<StreamMessage>) {
        for connection_id in self.subscribers.keys().copied().collect::<Vec<_>>() {
            if !self.egress.send(connection_id, message.clone()).await {
                self.subscribers.remove(&connection_id);
            }
        }
    }

    async fn send_to_subscriber(&mut self, connection_id: Uuid, message: Arc<StreamMessage>, version: i64) {
        if self.egress.send(connection_id, message).await {
            self.subscribers.insert(connection_id, Some(version));
        } else {
            self.subscribers.remove(&connection_id);
        }
    }
}

#[cfg(test)]
#[path = "stream_relay_test.rs"]
mod stream_relay_test;
