use shine_game::replica::StreamId;
use std::{cmp::Ordering, fmt};
use uuid::Uuid;

/// The position of an entry in a stream's inbox: the Redis stream id `<ms>-<seq>`. Ordered
/// numerically, which the string form is not (`"10-0"` sorts before `"9-0"`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InboxId {
    ms: u64,
    seq: u64,
}

impl InboxId {
    /// Before every entry: polling after it reads the inbox from its start.
    pub const START: InboxId = InboxId { ms: 0, seq: 0 };

    pub fn parse(value: &str) -> Option<Self> {
        let (ms, seq) = value.split_once('-')?;
        Some(Self {
            ms: ms.parse().ok()?,
            seq: seq.parse().ok()?,
        })
    }

    /// The smallest id above this one.
    pub fn next(&self) -> Self {
        match self.seq.checked_add(1) {
            Some(seq) => Self { ms: self.ms, seq },
            None => Self {
                ms: self.ms.saturating_add(1),
                seq: 0,
            },
        }
    }
}

impl Ord for InboxId {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.ms, self.seq).cmp(&(other.ms, other.seq))
    }
}

impl PartialOrd for InboxId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for InboxId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.ms, self.seq)
    }
}

/// A command as the client sent it, waiting to be sequenced: no version yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboxEntry {
    pub inbox_id: InboxId,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub data: String,
}

/// The result of a poll.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboxBatch {
    pub entries: Vec<InboxEntry>,
    pub cursor: InboxId,
}

/// A hint that a stream may have something to do. The notifications are not reliable (a reconnect
/// of the listener loses the ones sent meanwhile), so a consumer also polls on its own timer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InboxWake {
    /// A command arrived (`persisted` is `None`), or an instance persisted the stream up to a version.
    Stream {
        stream_id: StreamId,
        persisted: Option<i64>,
    },
    /// Notifications may have been lost: every stream should look.
    All,
}
