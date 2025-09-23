use crate::map::{BoxedMapLayerOperation, MapAuditedLayer, MapChunkId, MapLayer, MapLayerChecksum, MapLayerVersion};
use bevy::ecs::event::Event;
use core::fmt;
use shine_core::utils::simple_type_name;
use std::marker::PhantomData;

/// Control commands to bind external logic to layer lifecycle.
#[derive(Event)]
pub enum MapLayerControlEvent<L>
where
    L: MapLayer,
{
    /// Request to track the given layer
    Track(MapChunkId, PhantomData<L>),

    /// Request to untrack the given layer
    Untrack(MapChunkId),

    /// Request to store or validate a snapshot.
    Snapshot {
        id: MapChunkId,
        version: MapLayerVersion,
        checksum: MapLayerChecksum,
        snapshot: Option<Vec<u8>>,
    },
}

impl<L> fmt::Debug for MapLayerControlEvent<L>
where
    L: MapLayer,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::", simple_type_name::<Self>())?;

        match self {
            MapLayerControlEvent::Track(chunk_id, _) => {
                write!(f, "Track({chunk_id:?})")
            }
            MapLayerControlEvent::Untrack(chunk_id) => {
                write!(f, "Untrack({chunk_id:?})")
            }
            MapLayerControlEvent::Snapshot {
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

/// Sync commands targeting the layer.
#[derive(Event)]
#[allow(clippy::large_enum_variant)]
pub enum MapLayerSyncEvent<L>
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

impl<L> fmt::Debug for MapLayerSyncEvent<L>
where
    L: MapAuditedLayer,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::", simple_type_name::<Self>())?;

        match self {
            MapLayerSyncEvent::Initial { id } => {
                write!(f, "Initial({id:?})")?;
            }
            MapLayerSyncEvent::Snapshot { id, version, checksum, .. } => {
                write!(f, "Snapshot({id:?}, {version:?}, {checksum:?})")?;
            }
            MapLayerSyncEvent::Update { id, version, operation } => {
                write!(f, "Update({id:?}, {version:?}, op={})", operation.name())?;
            }
        }
        Ok(())
    }
}
