use crate::{
    repositories::{
        command_inbox::CommandInboxDb,
        command_log::{CommandLogDb, StoredCommand, StreamId},
    },
    services::command_stream::{
        CommandStreamService, StreamCommand, StreamEgress, StreamMessage, StreamSnapshot, StreamState,
    },
};
use shine_game::replica::ReplayState;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, Notify},
    time::{self, Instant, MissedTickBehavior},
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct WorkerConfig {
    /// Interval of the tick that looks at the inbox and the log even when no wake came, the safety net
    /// for a wake that was lost.
    pub backstop: Duration,
    /// Least time between two passes triggered by wakes, so a steady flow of commands is worked in
    /// batches.
    pub min_wake_spacing: Duration,
    /// How long the worker stays loaded after its last subscriber left.
    pub unload_grace: Duration,
    /// How many passes in a row may fail before the stream is given up.
    pub max_failures: u32,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            backstop: Duration::from_secs(10),
            min_wake_spacing: Duration::from_millis(50),
            unload_grace: Duration::from_secs(60),
            max_failures: 5,
        }
    }
}

enum Message {
    Subscribe(Uuid),
    Unsubscribe(Uuid),
    Stop,
}

/// How to reach a running worker. It does not depend on the kind of the stream, so the handles of
/// all the workers fit in one map.
#[derive(Clone)]
pub struct WorkerHandle {
    stream_id: StreamId,
    messages: mpsc::UnboundedSender<Message>,
    wake: Arc<Notify>,
}

impl WorkerHandle {
    pub fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }

    /// Adds the connection as a subscriber, or syncs it again if it already is one. `false` if the
    /// worker has ended: the caller has to start a new one.
    pub fn subscribe(&self, connection_id: Uuid) -> bool {
        self.messages.send(Message::Subscribe(connection_id)).is_ok()
    }

    pub fn unsubscribe(&self, connection_id: Uuid) -> bool {
        self.messages.send(Message::Unsubscribe(connection_id)).is_ok()
    }

    /// Tells the worker that there may be something new in the inbox or the log. Wakes collapse into
    /// one pass.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// Asks the worker to persist what is waiting and end.
    pub fn stop(&self) -> bool {
        self.messages.send(Message::Stop).is_ok()
    }

    pub fn is_finished(&self) -> bool {
        self.messages.is_closed()
    }
}

enum Flow {
    Continue,
    /// The log contradicts the state: load the stream again.
    Reload,
    /// The stream cannot be served.
    GiveUp,
}

/// Starts the worker of one stream. It loads the stream, then serves its subscribers until the last one
/// has left for the grace period, or it is stopped.
pub fn spawn_worker<S, L, I, E>(
    service: CommandStreamService<L, I>,
    egress: E,
    stream_id: StreamId,
    config: WorkerConfig,
) -> WorkerHandle
where
    S: ReplayState,
    L: CommandLogDb + 'static,
    I: CommandInboxDb + 'static,
    E: StreamEgress,
{
    let (messages, receiver) = mpsc::unbounded_channel();
    let wake = Arc::new(Notify::new());
    let worker = StreamWorker::<S, L, I, E> {
        service,
        egress,
        stream_id: stream_id.clone(),
        config,
        wake: wake.clone(),
        state: None,
        subscribers: HashMap::new(),
        failures: 0,
        grace_deadline: None,
    };
    tokio::spawn(worker.run(receiver));
    WorkerHandle { stream_id, messages, wake }
}

struct StreamWorker<S: ReplayState, L: CommandLogDb, I: CommandInboxDb, E: StreamEgress> {
    service: CommandStreamService<L, I>,
    egress: E,
    stream_id: StreamId,
    config: WorkerConfig,
    wake: Arc<Notify>,
    state: Option<StreamState<S>>,
    /// The subscribers, with the last version sent to each.
    subscribers: HashMap<Uuid, i64>,
    failures: u32,
    /// When the worker ends if it has no subscriber by then.
    grace_deadline: Option<Instant>,
}

impl<S, L, I, E> StreamWorker<S, L, I, E>
where
    S: ReplayState,
    L: CommandLogDb + 'static,
    I: CommandInboxDb + 'static,
    E: StreamEgress,
{
    async fn run(mut self, mut receiver: mpsc::UnboundedReceiver<Message>) {
        self.grace_deadline = Some(Instant::now() + self.config.unload_grace);
        match self.service.load::<S>(&self.stream_id).await {
            Ok(state) => self.state = Some(state),
            Err(err) => {
                tracing::warn!("[{}] Failed to load the stream: {err:#?}", self.stream_id);
                self.reject_queued(&mut receiver).await;
                return;
            }
        }

        let mut backstop = time::interval(self.config.backstop);
        backstop.set_missed_tick_behavior(MissedTickBehavior::Delay);
        backstop.tick().await;
        let wake = self.wake.clone();
        let mut next_wake_pass = Instant::now();

        // Commands that arrived while loading are worked at once.
        let mut alive = self.pass().await;
        while alive {
            let deadline = self.grace_deadline;
            let grace = async {
                match deadline {
                    Some(deadline) => time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                biased;
                message = receiver.recv() => match message {
                    Some(Message::Subscribe(connection_id)) => self.subscribe(connection_id).await,
                    Some(Message::Unsubscribe(connection_id)) => self.unsubscribe(connection_id),
                    Some(Message::Stop) | None => {
                        self.pass().await;
                        alive = false;
                    }
                },
                _ = wake.notified() => {
                    time::sleep_until(next_wake_pass).await;
                    alive = self.pass().await;
                    next_wake_pass = Instant::now() + self.config.min_wake_spacing;
                }
                _ = backstop.tick() => alive = self.pass().await,
                _ = grace => {
                    // The last words: whatever is waiting in the inbox is persisted.
                    self.pass().await;
                    alive = false;
                }
            }
        }
        if self.state.is_none() {
            self.reject_queued(&mut receiver).await;
        }
    }

    /// One look at the inbox and the log. Returns whether the worker goes on.
    async fn pass(&mut self) -> bool {
        let Some(mut state) = self.state.take() else {
            return false;
        };
        match self.work(&mut state).await {
            Flow::Continue => {
                self.state = Some(state);
                true
            }
            Flow::Reload => self.reload().await,
            Flow::GiveUp => {
                self.give_up().await;
                false
            }
        }
    }

    async fn work(&mut self, state: &mut StreamState<S>) -> Flow {
        match self.service.sync(state).await {
            Ok(outcome) if outcome.reload => return Flow::Reload,
            Ok(outcome) => self.deliver(outcome.confirmed).await,
            Err(err) => return self.failed(err),
        }
        match self.service.replay(state).await {
            Ok(outcome) if outcome.reload => return Flow::Reload,
            Ok(outcome) => {
                if let Some(checkpoint) = outcome.checkpoint {
                    let message = StreamMessage::Checkpoint {
                        stream_id: self.stream_id.clone(),
                        version: checkpoint.version,
                        state_hash: checkpoint.state_hash,
                    };
                    self.broadcast(Arc::new(message)).await;
                }
            }
            Err(err) => return self.failed(err),
        }
        self.failures = 0;
        Flow::Continue
    }

    fn failed(&mut self, err: impl std::fmt::Debug) -> Flow {
        self.failures += 1;
        tracing::warn!(
            "[{}] The pass failed ({} in a row): {err:#?}",
            self.stream_id,
            self.failures
        );
        if self.failures >= self.config.max_failures {
            Flow::GiveUp
        } else {
            Flow::Continue
        }
    }

    /// Loads the stream again and syncs every subscriber from it.
    async fn reload(&mut self) -> bool {
        match self.service.load::<S>(&self.stream_id).await {
            Ok(state) => {
                tracing::info!("[{}] The stream is reloaded", self.stream_id);
                self.failures = 0;
                let reset = Arc::new(StreamMessage::Reset {
                    stream_id: self.stream_id.clone(),
                });
                self.broadcast(reset).await;
                let sync = Arc::new(sync_message(&state));
                let version = state.confirmed_version();
                self.state = Some(state);
                for connection_id in self.subscriber_ids() {
                    self.send_to_subscriber(connection_id, sync.clone(), version).await;
                }
                self.arm_grace_if_empty();
                true
            }
            Err(err) => {
                tracing::warn!("[{}] Failed to reload the stream: {err:#?}", self.stream_id);
                self.give_up().await;
                false
            }
        }
    }

    async fn give_up(&mut self) {
        let message = Arc::new(StreamMessage::Unsynced {
            stream_id: self.stream_id.clone(),
        });
        self.broadcast(message).await;
        self.subscribers.clear();
        self.state = None;
    }

    /// Answers the subscribers that queued up while the stream was loading or being given up.
    async fn reject_queued(&mut self, receiver: &mut mpsc::UnboundedReceiver<Message>) {
        let message = Arc::new(StreamMessage::Unsynced {
            stream_id: self.stream_id.clone(),
        });
        receiver.close();
        while let Some(queued) = receiver.recv().await {
            if let Message::Subscribe(connection_id) = queued {
                self.egress.send(connection_id, message.clone()).await;
            }
        }
    }

    async fn subscribe(&mut self, connection_id: Uuid) {
        self.grace_deadline = None;
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let version = state.confirmed_version();
        let sync = Arc::new(sync_message(state));
        self.send_to_subscriber(connection_id, sync, version).await;
        self.arm_grace_if_empty();
    }

    fn unsubscribe(&mut self, connection_id: Uuid) {
        self.subscribers.remove(&connection_id);
        self.arm_grace_if_empty();
    }

    /// Sends the newly confirmed commands to the subscribers that have not got them.
    async fn deliver(&mut self, confirmed: Vec<StoredCommand>) {
        let (Some(first), Some(last)) = (confirmed.first(), confirmed.last()) else {
            return;
        };
        let (first, last) = (first.version, last.version);
        let all = Arc::new(StreamMessage::Commands {
            stream_id: self.stream_id.clone(),
            commands: confirmed.iter().map(StreamCommand::from).collect(),
        });
        for connection_id in self.subscriber_ids() {
            let sent_up_to = self.subscribers[&connection_id];
            let message = if sent_up_to + 1 == first {
                all.clone()
            } else {
                let commands: Vec<_> = confirmed
                    .iter()
                    .filter(|command| command.version > sent_up_to)
                    .map(StreamCommand::from)
                    .collect();
                if commands.is_empty() {
                    continue;
                }
                Arc::new(StreamMessage::Commands {
                    stream_id: self.stream_id.clone(),
                    commands,
                })
            };
            self.send_to_subscriber(connection_id, message, last).await;
        }
        self.arm_grace_if_empty();
    }

    async fn broadcast(&mut self, message: Arc<StreamMessage>) {
        for connection_id in self.subscriber_ids() {
            if !self.egress.send(connection_id, message.clone()).await {
                self.subscribers.remove(&connection_id);
            }
        }
        self.arm_grace_if_empty();
    }

    /// Sends a message that brings the subscriber up to `version`; a subscriber that cannot be sent to
    /// is forgotten.
    async fn send_to_subscriber(&mut self, connection_id: Uuid, message: Arc<StreamMessage>, version: i64) {
        if self.egress.send(connection_id, message).await {
            self.subscribers.insert(connection_id, version);
        } else {
            self.subscribers.remove(&connection_id);
        }
    }

    fn subscriber_ids(&self) -> Vec<Uuid> {
        self.subscribers.keys().copied().collect()
    }

    fn arm_grace_if_empty(&mut self) {
        if self.subscribers.is_empty() && self.grace_deadline.is_none() {
            self.grace_deadline = Some(Instant::now() + self.config.unload_grace);
        }
    }
}

/// The state a subscriber starts from: the latest snapshot and the confirmed commands after it.
fn sync_message<S: ReplayState>(state: &StreamState<S>) -> StreamMessage {
    StreamMessage::Sync {
        stream_id: state.stream_id().clone(),
        snapshot: state.snapshot().map(StreamSnapshot::from),
        commands: state.tail().iter().map(StreamCommand::from).collect(),
    }
}

#[cfg(test)]
#[path = "stream_worker_test.rs"]
mod stream_worker_test;
