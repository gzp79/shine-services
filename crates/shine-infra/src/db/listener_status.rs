use serde::Serialize;

/// State of the dedicated pub/sub connection of a pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenerStatus {
    /// A connection is currently established. A listener without channels stays disconnected.
    pub connected: bool,
    /// Number of channels with a registered handler.
    pub channels: usize,
}
