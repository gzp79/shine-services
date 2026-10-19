use crate::services::command_stream::{CommandStreamService, StreamEgress, StreamWorker, WorkerFuture, WorkerHandle};
use shine_game::replay::{decode_command, encode_command, ReplayState};
use shine_infra::db::command_log::StreamId;
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

/// Type-erased stream kind, so the hub and the routes stay non-generic.
pub trait StreamKind: Send + Sync + 'static {
    fn kind(&self) -> &'static str;

    /// The canonical JSON of `command` when it is a well-formed, valid command of this kind. Storing the
    /// re-serialized typed command drops unknown fields and keeps replay byte for byte deterministic.
    fn canonicalize(&self, command: &str) -> Option<String>;

    /// The worker replaying a stream of this kind on this instance.
    fn create_worker(
        &self,
        egress: Arc<dyn StreamEgress>,
        streams: CommandStreamService,
        stream_id: StreamId,
    ) -> (WorkerHandle, WorkerFuture);
}

pub struct ReplayKind<S>(PhantomData<fn() -> S>);

impl<S: ReplayState> StreamKind for ReplayKind<S> {
    fn kind(&self) -> &'static str {
        S::KIND
    }

    fn canonicalize(&self, command: &str) -> Option<String> {
        let command = decode_command::<S>(command).ok()?;
        if !S::validate(&command) {
            return None;
        }
        encode_command::<S>(&command).ok()
    }

    fn create_worker(
        &self,
        egress: Arc<dyn StreamEgress>,
        streams: CommandStreamService,
        stream_id: StreamId,
    ) -> (WorkerHandle, WorkerFuture) {
        StreamWorker::<S>::create(egress, streams, stream_id)
    }
}

#[derive(Clone, Default)]
pub struct StreamKindRegistry(HashMap<&'static str, Arc<dyn StreamKind>>);

impl StreamKindRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with<S: ReplayState>(mut self) -> Self {
        self.0.insert(S::KIND, Arc::new(ReplayKind::<S>(PhantomData)));
        self
    }

    /// Registers a kind that is not a plain `ReplayState`.
    #[cfg(test)]
    pub fn with_kind(mut self, kind: Arc<dyn StreamKind>) -> Self {
        self.0.insert(kind.kind(), kind);
        self
    }

    pub fn get(&self, kind: &str) -> Option<Arc<dyn StreamKind>> {
        self.0.get(kind).cloned()
    }
}
