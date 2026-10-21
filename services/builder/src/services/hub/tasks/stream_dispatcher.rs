use crate::{
    models::messages::{HubEvent, HubMessage, StreamEvent, StreamMessage, StreamResume, TopicKey},
    services::{
        command_stream::{CommandStreamService, StreamEgress, WorkerCommand, WorkerExit, WorkerHandle},
        HubService,
    },
};
use shine_infra::db::command_log::StreamId;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, Notify},
    task::{self, JoinError, JoinHandle, JoinSet},
    time,
};
use uuid::Uuid;

/// Min time between two starts of a stream's worker, so a worker that panics on start cannot spin.
const RESPAWN_DELAY: Duration = Duration::from_secs(1);

/// stream → worker wake, read by the Redis listener callback, which must never block or await.
#[derive(Clone, Default)]
struct WakeMap(Arc<RwLock<HashMap<StreamId, Arc<Notify>>>>);

impl WakeMap {
    fn insert(&self, stream_id: StreamId, wake: Arc<Notify>) {
        self.0
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(stream_id, wake);
    }

    fn remove(&self, stream_id: &StreamId) {
        self.0.write().unwrap_or_else(|p| p.into_inner()).remove(stream_id);
    }

    /// Wakes one stream, or every stream on `None` (listener reconnect).
    fn wake(&self, stream_id: Option<StreamId>) {
        let map = self.0.read().unwrap_or_else(|p| p.into_inner());
        match stream_id {
            Some(stream_id) => {
                if let Some(wake) = map.get(&stream_id) {
                    wake.notify_one();
                }
            }
            None => map.values().for_each(|wake| wake.notify_one()),
        }
    }
}

struct Resident {
    kind: &'static str,
    handle: WorkerHandle,
    task: task::Id,
    subscribers: HashSet<Uuid>,
    idle_since: Option<Instant>,
    spawned_at: Instant,
    /// The worker ended while the stream was still resident; restarted by `on_tick`.
    dead: bool,
    /// The stream cannot be replayed: new subscribers are refused until it is unloaded.
    unavailable: bool,
}

/// Hub task owning stream residency on this instance: one worker per stream with local subscribers.
pub struct StreamDispatcher {
    egress: Arc<dyn StreamEgress>,
    streams: CommandStreamService,
    wakes: WakeMap,
    residents: HashMap<StreamId, Resident>,
    by_connection: HashMap<Uuid, HashSet<StreamId>>,
    workers: JoinSet<WorkerExit>,
    sweep: Option<JoinHandle<()>>,
    last_sweep: Instant,
}

impl Drop for StreamDispatcher {
    /// Detaches the workers instead of aborting them: also when the hub aborts this task first, each
    /// worker sees its command channel close and runs its final persist.
    fn drop(&mut self) {
        self.workers.detach_all();
    }
}

impl StreamDispatcher {
    /// Subscribes to the hub events before returning, so no subscription is missed.
    pub async fn start(hub: HubService, streams: CommandStreamService) -> JoinHandle<()> {
        let subscription = hub.subscribe(vec![TopicKey::Hub, TopicKey::Stream]).await;
        let wakes = WakeMap::default();
        let listener = wakes.clone();
        if let Err(err) = streams.listen_to_wakes(move |stream_id| listener.wake(stream_id)).await {
            tracing::error!("Failed to listen for stream wakes; falling back to periodic sync only: {err:#?}");
        }
        let dispatcher = Self {
            egress: Arc::new(hub),
            streams,
            wakes,
            residents: HashMap::new(),
            by_connection: HashMap::new(),
            workers: JoinSet::new(),
            sweep: None,
            last_sweep: Instant::now(),
        };
        tokio::spawn(dispatcher.run(subscription))
    }

    async fn run(mut self, mut subscription: mpsc::UnboundedReceiver<HubMessage>) {
        let mut ticker = time::interval(self.streams.settings().sync_interval);
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                biased;
                _ = ticker.tick() => self.on_tick(),
                message = subscription.recv() => match message {
                    Some(HubMessage::StreamEvent(StreamEvent::Subscribed { connection_id, stream_id, kind, resume })) => {
                        self.subscribe(connection_id, stream_id, kind, resume).await
                    }
                    Some(HubMessage::StreamEvent(StreamEvent::Unsubscribed { connection_id, stream_id })) => {
                        self.unsubscribe(connection_id, &stream_id)
                    }
                    Some(HubMessage::Hub(HubEvent::UserDisconnected { connection_id, .. })) => self.drop_connection(connection_id),
                    Some(HubMessage::Hub(HubEvent::Shutdown)) | None => break,
                    Some(_) => {}
                },
                Some(exited) = self.workers.join_next_with_id() => self.on_worker_exit(exited).await,
            }
        }
        if let Some(sweep) = self.sweep.take() {
            sweep.abort();
        }
        tracing::info!("Stream dispatcher stopped");
        // Dropping `self` detaches the workers and closes their command channels: they persist and exit.
    }

    fn spawn_worker(&mut self, stream_id: &StreamId, kind: &'static str) -> Option<Resident> {
        let Some(stream_kind) = self.streams.kinds().get(kind) else {
            tracing::error!("[{stream_id}] Unknown stream kind {kind}");
            return None;
        };
        let (handle, future) = stream_kind.create_worker(self.egress.clone(), self.streams.clone(), stream_id.clone());
        let task = self.workers.spawn(future).id();
        self.wakes.insert(stream_id.clone(), handle.wake.clone());
        Some(Resident {
            kind,
            handle,
            task,
            subscribers: HashSet::new(),
            idle_since: None,
            spawned_at: Instant::now(),
            dead: false,
            unavailable: false,
        })
    }

    async fn refuse(&self, connection_id: Uuid, stream_id: StreamId) {
        let message = StreamMessage::SubscribeFailed {
            stream_id,
            reason: "unavailable",
        };
        // A connection that cannot take the message is dropped by the hub.
        self.egress.send(connection_id, message).await;
    }

    async fn subscribe(
        &mut self,
        connection_id: Uuid,
        stream_id: StreamId,
        kind: &'static str,
        resume: Option<StreamResume>,
    ) {
        if !self.residents.contains_key(&stream_id) {
            let Some(resident) = self.spawn_worker(&stream_id, kind) else {
                self.refuse(connection_id, stream_id).await;
                return;
            };
            self.residents.insert(stream_id.clone(), resident);
        }
        let Some(resident) = self.residents.get_mut(&stream_id) else {
            return;
        };
        if resident.unavailable {
            self.refuse(connection_id, stream_id).await;
            return;
        }
        resident.subscribers.insert(connection_id);
        resident.idle_since = None;
        let _ = resident
            .handle
            .tx
            .send(WorkerCommand::AddSubscriber(connection_id, resume));
        self.by_connection.entry(connection_id).or_default().insert(stream_id);
    }

    fn unsubscribe(&mut self, connection_id: Uuid, stream_id: &StreamId) {
        if let Some(streams) = self.by_connection.get_mut(&connection_id) {
            streams.remove(stream_id);
            if streams.is_empty() {
                self.by_connection.remove(&connection_id);
            }
        }
        if let Some(resident) = self.residents.get_mut(stream_id) {
            if resident.subscribers.remove(&connection_id) {
                let _ = resident.handle.tx.send(WorkerCommand::RemoveSubscriber(connection_id));
            }
            if resident.subscribers.is_empty() && resident.idle_since.is_none() {
                resident.idle_since = Some(Instant::now());
            }
        }
    }

    fn drop_connection(&mut self, connection_id: Uuid) {
        for stream_id in self.by_connection.remove(&connection_id).unwrap_or_default() {
            self.unsubscribe(connection_id, &stream_id);
        }
    }

    fn on_tick(&mut self) {
        self.respawn_dead();
        let grace = self.streams.settings().unload_grace;
        let idle: Vec<StreamId> = self
            .residents
            .iter()
            .filter(|(_, r)| r.idle_since.is_some_and(|since| since.elapsed() >= grace))
            .map(|(stream_id, _)| stream_id.clone())
            .collect();
        for stream_id in idle {
            if let Some(resident) = self.residents.remove(&stream_id) {
                self.wakes.remove(&stream_id);
                let _ = resident.handle.tx.send(WorkerCommand::Stop);
                tracing::debug!("[{stream_id}] Stream unloaded");
            }
        }

        let sweep_due = self.last_sweep.elapsed() >= self.streams.settings().orphan_sweep_interval;
        let sweep_idle = self.sweep.as_ref().is_none_or(|sweep| sweep.is_finished());
        if sweep_due && sweep_idle {
            self.last_sweep = Instant::now();
            let streams = self.streams.clone();
            self.sweep = Some(tokio::spawn(async move { streams.sweep_orphans().await }));
        }
    }

    /// Handles the end of a worker of a still-resident stream: a stream that cannot be replayed refuses
    /// its subscribers, any other end is restarted by `on_tick`.
    async fn on_worker_exit(&mut self, exited: Result<(task::Id, WorkerExit), JoinError>) {
        let (id, exit, panicked) = match exited {
            Ok((id, exit)) => (id, Some(exit), false),
            Err(err) => (err.id(), None, err.is_panic()),
        };
        // An unloaded or replaced worker is not a resident anymore.
        let Some((stream_id, resident)) = self.residents.iter_mut().find(|(_, r)| r.task == id) else {
            return;
        };
        let stream_id = stream_id.clone();

        if exit == Some(WorkerExit::Unavailable) {
            tracing::error!("[{stream_id}] Stream unavailable, refusing its subscribers");
            resident.unavailable = true;
            resident.idle_since = Some(Instant::now());
            let subscribers = std::mem::take(&mut resident.subscribers);
            self.wakes.remove(&stream_id);
            for connection_id in subscribers {
                if let Some(streams) = self.by_connection.get_mut(&connection_id) {
                    streams.remove(&stream_id);
                    if streams.is_empty() {
                        self.by_connection.remove(&connection_id);
                    }
                }
                self.refuse(connection_id, stream_id.clone()).await;
            }
            return;
        }

        if panicked {
            tracing::error!("[{stream_id}] Stream worker panicked");
        } else {
            tracing::warn!("[{stream_id}] Stream worker ended unexpectedly");
        }
        resident.dead = true;
    }

    /// Restarts dead workers, at most once per `RESPAWN_DELAY` per stream.
    fn respawn_dead(&mut self) {
        let due: Vec<StreamId> = self
            .residents
            .iter()
            .filter(|(_, r)| r.dead && r.spawned_at.elapsed() >= RESPAWN_DELAY)
            .map(|(stream_id, _)| stream_id.clone())
            .collect();
        for stream_id in due {
            let Some(old) = self.residents.remove(&stream_id) else {
                continue;
            };
            self.wakes.remove(&stream_id);
            if old.subscribers.is_empty() {
                continue;
            }
            if let Some(mut resident) = self.spawn_worker(&stream_id, old.kind) {
                for connection_id in &old.subscribers {
                    let _ = resident
                        .handle
                        .tx
                        .send(WorkerCommand::AddSubscriber(*connection_id, None));
                }
                resident.subscribers = old.subscribers;
                self.residents.insert(stream_id, resident);
            }
        }
    }
}
