use crate::map::{BoxedMapLayerOperation, MapAuditedLayer, MapChunkId, MapLayerChecksum, MapLayerVersion};
use bevy::ecs::message::Message;
use shine_core::utils::simple_type_name;
use std::fmt;

/// Message to request some action on the map servers.
/// These messages are usually sent to the servers.
#[derive(Message)]
#[allow(clippy::large_enum_variant)]
pub enum MapLayerActionMessage<L>
where
    L: MapAuditedLayer,
{
    /// Request to track the given layer
    Track(MapChunkId),

    /// Request to untrack the given layer
    Untrack(MapChunkId),

    /// Request an operation on the layer.
    Update {
        id: MapChunkId,
        operation: BoxedMapLayerOperation<L>,
    },

    /// Request to store or validate a snapshot.
    Snapshot {
        id: MapChunkId,
        version: MapLayerVersion,
        checksum: MapLayerChecksum,
        snapshot: Option<Vec<u8>>,
    },
}

impl<L> Clone for MapLayerActionMessage<L>
where
    L: MapAuditedLayer,
{
    fn clone(&self) -> Self {
        match self {
            Self::Track(chunk_id) => Self::Track(*chunk_id),
            Self::Untrack(chunk_id) => Self::Untrack(*chunk_id),
            Self::Update { id, operation } => Self::Update {
                id: *id,
                operation: operation.boxed_clone(),
            },
            Self::Snapshot {
                id,
                version,
                checksum,
                snapshot,
            } => Self::Snapshot {
                id: *id,
                version: *version,
                checksum: *checksum,
                snapshot: snapshot.clone(),
            },
        }
    }
}

impl<L> fmt::Debug for MapLayerActionMessage<L>
where
    L: MapAuditedLayer,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::", simple_type_name::<Self>())?;

        match self {
            MapLayerActionMessage::Track(chunk_id) => {
                write!(f, "Track({chunk_id:?})")
            }
            MapLayerActionMessage::Untrack(chunk_id) => {
                write!(f, "Untrack({chunk_id:?})")
            }
            MapLayerActionMessage::Update { id, operation } => {
                write!(f, "Update({id:?}, op={})", operation.name())
            }
            MapLayerActionMessage::Snapshot {
                id,
                version,
                checksum,
                snapshot,
            } => {
                write!(
                    f,
                    "Snapshot({id:?}, {version:?}, {checksum:?}, snapshot={})",
                    if snapshot.is_some() { "Some(...)" } else { "None" }
                )
            }
        }
    }
}

/// Notification messages that some state has changed on the map.
/// These messages are usually sent to the clients.
#[derive(Message)]
#[allow(clippy::large_enum_variant)]
pub enum MapLayerNotificationMessage<L>
where
    L: MapAuditedLayer,
{
    /// A new layer that is not persisted yet.
    Initial { id: MapChunkId },

    /// A full (authentic) snapshot of the layer.
    Snapshot {
        id: MapChunkId,
        version: MapLayerVersion,
        checksum: MapLayerChecksum,
        snapshot: Vec<u8>,
    },

    /// An operation to be applied to the layer.
    Update {
        id: MapChunkId,
        version: MapLayerVersion,
        operation: BoxedMapLayerOperation<L>,
    },
}

impl<L> Clone for MapLayerNotificationMessage<L>
where
    L: MapAuditedLayer,
{
    fn clone(&self) -> Self {
        match self {
            Self::Initial { id } => Self::Initial { id: *id },
            Self::Snapshot {
                id,
                version,
                checksum,
                snapshot,
            } => Self::Snapshot {
                id: *id,
                version: *version,
                checksum: *checksum,
                snapshot: snapshot.clone(),
            },
            Self::Update { id, version, operation } => Self::Update {
                id: *id,
                version: *version,
                operation: operation.boxed_clone(),
            },
        }
    }
}

impl<L> fmt::Debug for MapLayerNotificationMessage<L>
where
    L: MapAuditedLayer,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::", simple_type_name::<Self>())?;

        match self {
            MapLayerNotificationMessage::Initial { id } => {
                write!(f, "Initial({id:?})")?;
            }
            MapLayerNotificationMessage::Snapshot { id, version, checksum, .. } => {
                write!(f, "Snapshot({id:?}, {version:?}, {checksum:?})")?;
            }
            MapLayerNotificationMessage::Update { id, version, operation } => {
                write!(f, "Update({id:?}, {version:?}, op={})", operation.name())?;
            }
        }
        Ok(())
    }
}
