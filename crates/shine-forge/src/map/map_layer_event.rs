use crate::map::{BoxedMapLayerOperation, MapAuditedLayer, MapChunkId, MapLayerChecksum, MapLayerVersion};
use bevy::ecs::event::Event;
use shine_core::utils::simple_type_name;
use std::fmt;

/// Event to request some action on the map servers.
/// These events are usually sent to the servers.
#[derive(Event)]
#[allow(clippy::large_enum_variant)]
pub enum MapLayerActionEvent<L>
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

impl<L> Clone for MapLayerActionEvent<L>
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

impl<L> fmt::Debug for MapLayerActionEvent<L>
where
    L: MapAuditedLayer,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::", simple_type_name::<Self>())?;

        match self {
            MapLayerActionEvent::Track(chunk_id) => {
                write!(f, "Track({chunk_id:?})")
            }
            MapLayerActionEvent::Untrack(chunk_id) => {
                write!(f, "Untrack({chunk_id:?})")
            }
            MapLayerActionEvent::Update { id, operation } => {
                write!(f, "Update({id:?}, op={})", operation.name())
            }
            MapLayerActionEvent::Snapshot {
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

/// Notification events that some state has changed on the map.
/// These events are usually sent to the clients.
#[derive(Event)]
#[allow(clippy::large_enum_variant)]
pub enum MapLayerNotificationEvent<L>
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

impl<L> Clone for MapLayerNotificationEvent<L>
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

impl<L> fmt::Debug for MapLayerNotificationEvent<L>
where
    L: MapAuditedLayer,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::", simple_type_name::<Self>())?;

        match self {
            MapLayerNotificationEvent::Initial { id } => {
                write!(f, "Initial({id:?})")?;
            }
            MapLayerNotificationEvent::Snapshot { id, version, checksum, .. } => {
                write!(f, "Snapshot({id:?}, {version:?}, {checksum:?})")?;
            }
            MapLayerNotificationEvent::Update { id, version, operation } => {
                write!(f, "Update({id:?}, {version:?}, op={})", operation.name())?;
            }
        }
        Ok(())
    }
}
