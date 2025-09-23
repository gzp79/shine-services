use crate::map::{
    map_layer_io::MapLayerIOExt, MapChunk, MapChunkId, MapLayerChecksum, MapLayerControlEvent, MapLayerIO,
    MapLayerInfo, MapLayerOf, MapLayerSyncEvent, MapLayerVersion,
};
use bevy::ecs::{
    component::{Component, Mutable},
    entity::Entity,
    event::{EventReader, EventWriter},
    query::{Added, Without},
    removal_detection::RemovedComponents,
    resource::Resource,
    system::{Commands, Local, Query, Res, ResMut},
    world::{FromWorld, World},
};
use std::{collections::HashMap, marker::PhantomData};

pub trait MapLayerConfig: Resource + Clone + Send + Sync + 'static {}

/// Trait to define a layer of a chunk.
pub trait MapLayer: Component<Mutability = Mutable> + 'static {
    type Config: MapLayerConfig;

    fn new() -> Self
    where
        Self: Sized;

    /// Clears the layer, resetting it to an empty and uninitialized state.
    fn clear(&mut self);

    /// Initializes the layer with the provided configuration, setting it to a default, ready-to-use state.
    /// This can be called multiple times to reconfigure the layer.
    fn initialize(&mut self, config: &Self::Config);

    /// Check if the layer is empty (i.e. cleared and has not been initialized).
    fn is_empty(&self) -> bool;
}

/// Map layer with change tracking capabilities.
/// The change tracking is operation dependent, but usually some dirty flag or a list of changed coordinates is used.
pub trait MapAuditedLayer: MapLayer {
    type Audit: MapLayer<Config = Self::Config>;
}

/// Resource to track a layer of a chunk.
#[derive(Resource)]
pub struct MapLayerTracker<L>
where
    L: MapLayer,
{
    chunks_to_entity: HashMap<MapChunkId, Entity>,
    entity_to_chunk: HashMap<Entity, MapChunkId>,
    ph: PhantomData<L>,
}

impl<L> Default for MapLayerTracker<L>
where
    L: MapLayer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<L> MapLayerTracker<L>
where
    L: MapLayer,
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

/// Configuration of the map layer systems.
/// It is registered as a resource, but usually converted into a local resource for the systems.
#[derive(Debug, Resource)]
pub struct MapLayerSystemConfig<L>
where
    L: MapLayer,
{
    pub initialize_spawned_layers: bool,
    pub process_sync_events: bool,
    pub full_snapshot_frequency: usize,
    _ph: PhantomData<L>,
}

impl<L> Clone for MapLayerSystemConfig<L>
where
    L: MapLayer,
{
    fn clone(&self) -> Self {
        Self {
            initialize_spawned_layers: self.initialize_spawned_layers,
            process_sync_events: self.process_sync_events,
            full_snapshot_frequency: self.full_snapshot_frequency,
            _ph: self._ph,
        }
    }
}

impl<L> MapLayerSystemConfig<L>
where
    L: MapLayer,
{
    pub fn server() -> Self {
        Self {
            initialize_spawned_layers: true,
            process_sync_events: true,
            full_snapshot_frequency: 10,
            _ph: PhantomData,
        }
    }

    pub fn client_authentic() -> Self {
        Self {
            initialize_spawned_layers: false,
            process_sync_events: true,
            full_snapshot_frequency: 0,
            _ph: PhantomData,
        }
    }

    pub fn client_local() -> Self {
        Self {
            initialize_spawned_layers: true,
            process_sync_events: false,
            full_snapshot_frequency: 0,
            _ph: PhantomData,
        }
    }
}

/// Local configuration and state for the `create_layer_as_child` system
pub struct CreateLayerState<L>
where
    L: MapLayer,
{
    initialize_spawned: bool,
    _ph: PhantomData<L>,
}

impl<L> FromWorld for CreateLayerState<L>
where
    L: MapLayer,
{
    fn from_world(world: &mut World) -> Self {
        let config = world
            .get_resource::<MapLayerSystemConfig<L>>()
            .expect("MapLayerSystemConfig<L> not found");
        Self {
            initialize_spawned: config.initialize_spawned_layers,
            _ph: PhantomData,
        }
    }
}

/// When a new chunk is created, this system creates layer components and performs some bookkeeping.
#[allow(clippy::type_complexity)]
pub fn create_layer_as_child<L>(
    layer_config: Res<L::Config>,
    mut layer_tracker: ResMut<MapLayerTracker<L>>,
    new_root_query: Query<(Entity, &MapChunk), (Added<MapChunk>, Without<L>)>,
    mut commands: Commands,
    mut replay_control: EventWriter<MapLayerControlEvent<L>>,
    system_config: Local<CreateLayerState<L>>,
) where
    L: MapLayer,
{
    for (root_entity, chunk_root) in new_root_query.iter() {
        log::debug!("Chunk [{:?}]: Create layer", chunk_root.id);

        // spawn the layer as a child of the chunk root
        let layer = {
            let version = MapLayerInfo::<L>::new();
            let mut layer = L::new();
            if system_config.initialize_spawned {
                layer.initialize(&layer_config);
            }
            (version, layer, MapLayerOf(root_entity))
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
pub fn remove_layer<L>(
    mut layer_tracker: ResMut<MapLayerTracker<L>>,
    mut removed_component: RemovedComponents<L>,
    mut replay_control: EventWriter<MapLayerControlEvent<L>>,
) where
    L: MapLayer,
{
    for entity in removed_component.read() {
        if let Some(chunk_id) = layer_tracker.entity_to_chunk.remove(&entity) {
            log::debug!("Chunk [{chunk_id:?}]: Remove layer");

            // Update the tracking info
            layer_tracker.chunks_to_entity.remove(&chunk_id);

            // Notify the replay system to untrack this layer
            replay_control.write(MapLayerControlEvent::Untrack(chunk_id));
        }
    }
}

/// Local configuration and state for the `process_layer_sync_events` system
pub struct ProcessLayerSyncEventState<L> {
    /// How often to create a full snapshot after an operation-based update.
    full_snapshot_frequency: usize,

    /// The next version number at which a full snapshot should be created.
    /// This is updated each time a full snapshot is created.
    next_snapshot_at: usize,

    _ph: PhantomData<L>,
}

impl<L> FromWorld for ProcessLayerSyncEventState<L>
where
    L: MapLayer,
{
    fn from_world(world: &mut World) -> Self {
        let config = world
            .get_resource::<MapLayerSystemConfig<L>>()
            .expect("MapLayerSystemConfig<L> not found");
        let mut state = Self {
            full_snapshot_frequency: config.full_snapshot_frequency,
            next_snapshot_at: usize::MAX,
            _ph: PhantomData,
        };
        state.update_next_snapshot(0);
        state
    }
}

impl<L> ProcessLayerSyncEventState<L>
where
    L: MapLayer,
{
    fn update_next_snapshot(&mut self, current: usize) {
        if self.full_snapshot_frequency > 0 {
            // todo: use some randomness here
            self.next_snapshot_at = current + self.full_snapshot_frequency;
        } else {
            self.next_snapshot_at = usize::MAX;
        }
    }
}

/// Process 'MapLayerSyncEvent' events.
pub fn process_layer_sync_events<L>(
    layer_config: Res<L::Config>,
    layer_tracker: ResMut<MapLayerTracker<L>>,
    mut layers: Query<(&mut MapLayerInfo<L>, &mut L, Option<&mut L::Audit>)>,
    mut sync_events: EventReader<MapLayerSyncEvent<L>>,
    mut control_events: EventWriter<MapLayerControlEvent<L>>,
    mut system_config: Local<ProcessLayerSyncEventState<L>>,
) where
    L: MapAuditedLayer + MapLayerIO,
    L::Config: Resource,
{
    for event in sync_events.read() {
        match event {
            MapLayerSyncEvent::Initial { id } => {
                log::debug!("Chunk [{id:?}]: Empty layer");
                if let Some((mut info, mut layer, mut layer_change)) =
                    layer_tracker.get_entity(*id).and_then(|e| layers.get_mut(e).ok())
                {
                    info.version = MapLayerVersion::new();
                    info.checksum = MapLayerChecksum::new();
                    layer.initialize(&layer_config);
                    if let Some(change) = layer_change.as_deref_mut() {
                        change.initialize(&layer_config);
                    }
                }
                system_config.update_next_snapshot(0);
            }
            MapLayerSyncEvent::Snapshot {
                id,
                version: evt_version,
                checksum: evt_checksum,
                snapshot,
            } => {
                log::debug!("Chunk [{id:?}]: Snapshot (version={evt_version:?}, {evt_checksum:?})");
                if let Some((mut info, mut layer, mut layer_change)) =
                    layer_tracker.get_entity(*id).and_then(|e| layers.get_mut(e).ok())
                {
                    info.version = *evt_version;
                    info.checksum = *evt_checksum;
                    if let Err(e) = layer.load_from_bytes(&layer_config, snapshot, layer_change.as_deref_mut()) {
                        log::error!("Chunk [{id:?}]: Failed to load layer data: {e}");
                        layer.clear();
                        info.version = MapLayerVersion::new();
                        info.checksum = MapLayerChecksum::new();
                        if let Some(change) = layer_change.as_deref_mut() {
                            change.clear();
                        }
                    } else {
                        system_config.update_next_snapshot(info.version.0);
                    }
                }
            }
            MapLayerSyncEvent::Update {
                id,
                version: evt_version,
                operation,
            } => {
                log::debug!("Chunk [{id:?}]: Update operation(op={})", operation.name());
                if let Some((mut info, mut layer, mut layer_change)) =
                    layer_tracker.get_entity(*id).and_then(|e| layers.get_mut(e).ok())
                {
                    if info.version.next() == *evt_version {
                        info.checksum = operation.apply(&mut layer, layer_change.as_deref_mut());
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
