use shine_game::replay::{decode_command, encode_command, ReplayState};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

/// Type-erased stream kind, so the hub and the routes stay non-generic.
pub trait StreamKind: Send + Sync + 'static {
    fn kind(&self) -> &'static str;

    /// The canonical JSON of `command` when it is a well-formed, valid command of this kind. Storing the
    /// re-serialized typed command drops unknown fields and keeps replay byte for byte deterministic.
    fn canonicalize(&self, command: &str) -> Option<String>;
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

    pub fn get(&self, kind: &str) -> Option<Arc<dyn StreamKind>> {
        self.0.get(kind).cloned()
    }
}
