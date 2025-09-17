use crate::map::{
    MapChunk, MapChunkId, MapError, MapLayerChecksum, MapLayerControlEvent, MapLayerInfo, MapLayerOf,
    MapLayerSyncEvent, MapLayerVersion, Tile,
};
use bevy::ecs::{
    component::{Component, Mutable},
    entity::Entity,
    event::{EventReader, EventWriter},
    query::{Added, Without},
    removal_detection::RemovedComponents,
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
};
use shine_core::utils::simple_type_name;
use std::{collections::HashMap, marker::PhantomData};

/// Trait to define a layer of a chunk.
pub trait MapLayer: Component<Mutability = Mutable> + 'static {
    type Tile: Tile;

    fn new_empty() -> Self
    where
        Self: Sized;

    fn is_empty(&self) -> bool;
    fn clear(&mut self);

    fn load(&mut self, data: &[u8]) -> Result<(), MapError>;
    fn save(&self) -> Vec<u8>;
}

/// Resource to track a layer of a chunk.
#[derive(Resource)]
pub struct MapLayerTracker<C>
where
    C: MapLayer,
{
    chunks_to_entity: HashMap<MapChunkId, Entity>,
    entity_to_chunk: HashMap<Entity, MapChunkId>,
    ph: PhantomData<C>,
}

impl<C> Default for MapLayerTracker<C>
where
    C: MapLayer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C> MapLayerTracker<C>
where
    C: MapLayer,
{
    pub fn new() -> Self {
        Self {
            chunks_to_entity: HashMap::new(),
            entity_to_chunk: HashMap::new(),
            ph: PhantomData,
        }
    }

    pub fn get_entity(&self, chunk_id: MapChunkId) -> Option<Entity> {
        self.chunks_to_entity.get(&chunk_id).cloned()
    }

    pub fn get_chunk_id(&self, root: Entity) -> Option<MapChunkId> {
        self.entity_to_chunk.get(&root).cloned()
    }
}

/// When a new chunk is created, this system creates layer components and performs some bookkeeping.
#[allow(clippy::type_complexity)]
pub fn create_layer_as_child<CFG, C>(
    layer_config: Res<CFG>,
    mut layer_tracker: ResMut<MapLayerTracker<C>>,
    new_root_query: Query<(Entity, &MapChunk), (Added<MapChunk>, Without<C>)>,
    mut commands: Commands,
    mut replay_control: EventWriter<MapLayerControlEvent<C>>,
) where
    CFG: Resource + Clone,
    C: MapLayer + From<CFG>,
{
    for (root_entity, chunk_root) in new_root_query.iter() {
        log::debug!(
            "Chunk [{:?}]: Create {} layer",
            chunk_root.id,
            simple_type_name::<C::Tile>()
        );

        // spawn a new child entity with the layer component
        let layer = {
            let version = MapLayerInfo::<C>::new();
            let chunk = C::from(layer_config.as_ref().clone());
            (version, chunk, MapLayerOf(root_entity))
        };
        let layer_entity = commands.spawn(layer).id();

        // Update the tracking info
        layer_tracker.chunks_to_entity.insert(chunk_root.id, layer_entity);
        layer_tracker.entity_to_chunk.insert(layer_entity, chunk_root.id);

        // Notify the replay system to track this layer
        replay_control.write(MapLayerControlEvent::Track(chunk_root.id, PhantomData));
    }
}

/// When a chunk is despawned, perform some cleanup.
pub fn remove_layer<C>(
    mut layer_tracker: ResMut<MapLayerTracker<C>>,
    mut removed_component: RemovedComponents<C>,
    mut replay_control: EventWriter<MapLayerControlEvent<C>>,
) where
    C: MapLayer,
{
    for entity in removed_component.read() {
        if let Some(chunk_id) = layer_tracker.entity_to_chunk.remove(&entity) {
            log::debug!("Chunk [{chunk_id:?}]: Remove {} layer", simple_type_name::<C::Tile>());

            // Update the tracking info
            layer_tracker.chunks_to_entity.remove(&chunk_id);

            // Notify the replay system to untrack this layer
            replay_control.write(MapLayerControlEvent::Untrack(chunk_id));
        }
    }
}

/// Process 'MapLayerSyncEvent' events.
pub fn process_layer_sync_events<C>(
    layer_tracker: ResMut<MapLayerTracker<C>>,
    mut layers: Query<(&mut MapLayerInfo<C>, &mut C)>,
    mut sync_events: EventReader<MapLayerSyncEvent<C>>,
    mut control_events: EventWriter<MapLayerControlEvent<C>>,
) where
    C: MapLayer,
{
    for event in sync_events.read() {
        match event {
            MapLayerSyncEvent::Snapshot {
                id,
                version: evt_version,
                checksum: evt_checksum,
                snapshot,
            } => {
                log::debug!("Chunk [{id:?}]: Snapshot (version={evt_version:?}, {evt_checksum:?})");
                if let Some((mut info, mut layer)) = layer_tracker.get_entity(*id).and_then(|e| layers.get_mut(e).ok())
                {
                    info.version = *evt_version;
                    info.checksum = *evt_checksum;
                    if let Err(e) = layer.load(snapshot) {
                        log::error!("Chunk [{id:?}]: Failed to load layer data: {e}");
                        layer.clear();
                        info.version = MapLayerVersion::new();
                        info.checksum = MapLayerChecksum::new();
                    }
                }
            }
            MapLayerSyncEvent::Update {
                id,
                version: evt_version,
                operation,
            } => {
                log::debug!("Chunk [{id:?}]: Update operation(op={})", operation.name());
                if let Some((mut info, mut layer)) = layer_tracker.get_entity(*id).and_then(|e| layers.get_mut(e).ok())
                {
                    if info.version.next() == *evt_version {
                        info.checksum = operation.apply(&mut layer);
                        info.version = *evt_version;
                    } else {
                        log::warn!(
                            "Chunk [{id:?}]: Ignored out-of-order operation (current version={:?}, operation version={:?})",
                            info.version,
                            *evt_version
                        );
                    }

                    control_events.write(MapLayerControlEvent::Snapshot {
                        id: *id,
                        version: info.version,
                        checksum: info.checksum,
                        snapshot: None,
                    });
                }
            }
        }
    }
}
