use crate::map::{
    MapAuditedLayer, MapChunk, MapLayer, MapLayerActionEvent, MapLayerChecksum, MapLayerClientChannels, MapLayerConfig,
    MapLayerIO, MapLayerIOExt, MapLayerInfo, MapLayerNotificationEvent, MapLayerOf, MapLayerTracker, MapLayerVersion,
    Tile,
};
use bevy::{
    ecs::{
        entity::Entity,
        event::{EventReader, EventWriter},
        query::{Added, Without},
        removal_detection::RemovedComponents,
        resource::Resource,
        system::{Commands, EntityCommands, Local, Query, Res, ResMut},
        world::{FromWorld, World},
    },
    log,
};
use shine_core::utils::simple_type_name;
use std::marker::PhantomData;

/// A shard of a map, consisting of a primary layer an optional overlay layer and a change tracking (audit) layer.
/// It is not required to spawn all the layer components, but these layers can operate together to provide a complete
/// map layer functionality for a specific tile type.
pub trait MapShard: Send + Sync + 'static {
    type Tile: Tile;
    type Config: MapLayerConfig;

    type Primary: MapLayer<Config = Self::Config> + MapAuditedLayer<Audit = Self::Audit> + MapLayerIO;
    type Overlay: MapLayer<Config = Self::Config> + MapAuditedLayer<Audit = Self::Audit>;
    type Audit: MapLayer<Config = Self::Config>;
}

/// Configuration of the map layer systems.
/// It is registered as a resource, but usually converted into a local resource for the systems.
#[derive(Resource)]
pub struct MapShardSystemConfig<S>
where
    S: MapShard,
{
    pub initialize_on_spawn: bool,
    pub enable_audit: bool,
    pub enable_overlay: bool,
    pub snapshot_frequency: usize,
    pub client_channels: Option<MapLayerClientChannels<S::Primary>>,
    _ph: PhantomData<S>,
}

impl<S> Clone for MapShardSystemConfig<S>
where
    S: MapShard,
{
    fn clone(&self) -> Self {
        Self {
            initialize_on_spawn: self.initialize_on_spawn,
            enable_audit: self.enable_audit,
            enable_overlay: self.enable_overlay,
            snapshot_frequency: self.snapshot_frequency,
            client_channels: None,
            _ph: self._ph,
        }
    }
}

impl<S> MapShardSystemConfig<S>
where
    S: MapShard,
{
    pub fn server() -> Self {
        Self {
            initialize_on_spawn: true,
            enable_audit: false,
            enable_overlay: false,
            snapshot_frequency: 10,
            client_channels: None,
            _ph: PhantomData,
        }
    }

    pub fn client() -> Self {
        Self {
            initialize_on_spawn: false,
            enable_audit: true,
            enable_overlay: true,
            snapshot_frequency: 0,
            client_channels: None,
            _ph: PhantomData,
        }
    }

    pub fn client_with_channels(client_channels: MapLayerClientChannels<S::Primary>) -> Self {
        Self {
            initialize_on_spawn: false,
            enable_audit: true,
            enable_overlay: true,
            snapshot_frequency: 0,
            client_channels: Some(client_channels),
            _ph: PhantomData,
        }
    }
}

/// Local configuration and state for the `create_shard` system
pub struct CreateShardState<S>
where
    S: MapShard,
{
    initialize_on_spawn: bool,
    enable_audit: bool,
    enable_overlay: bool,
    _ph: PhantomData<S>,
}

impl<S> FromWorld for CreateShardState<S>
where
    S: MapShard,
{
    fn from_world(world: &mut World) -> Self {
        let config = world.get_resource::<MapShardSystemConfig<S>>().unwrap_or_else(|| {
            panic!(
                "MapShardSystemConfig not found for {} layer",
                simple_type_name::<S::Tile>()
            )
        });
        Self {
            initialize_on_spawn: config.initialize_on_spawn,
            enable_audit: config.enable_audit,
            enable_overlay: config.enable_overlay,
            _ph: PhantomData,
        }
    }
}

impl<S> CreateShardState<S>
where
    S: MapShard,
{
    pub fn spawn<'c>(
        &self,
        commands: &'c mut Commands,
        chunk_root: Entity,
        layer_config: &S::Config,
    ) -> EntityCommands<'c> {
        // spawn the layer as a child of the chunk root
        let layer = {
            let version = MapLayerInfo::<S::Primary>::new();

            let mut layer = S::Primary::new();
            if self.initialize_on_spawn {
                layer.initialize(layer_config);
            }

            (version, layer, MapLayerOf(chunk_root))
        };
        let mut command = commands.spawn(layer);

        if self.enable_audit {
            let audit = {
                let mut layer = S::Audit::new();
                if self.initialize_on_spawn {
                    layer.initialize(layer_config);
                }
                layer
            };
            command.insert(audit);
        }

        if self.enable_overlay {
            let mut overlay = S::Overlay::new();
            if self.initialize_on_spawn {
                overlay.initialize(layer_config);
            }
            command.insert(overlay);
        }

        command
    }
}

/// Create layer components and performs some book-keeping
/// when a new chunk root is spawned.
#[allow(clippy::type_complexity)]
pub fn create_shard<S>(
    layer_config: Res<S::Config>,
    mut layer_tracker: ResMut<MapLayerTracker<S::Primary>>,
    new_root_query: Query<(Entity, &MapChunk), (Added<MapChunk>, Without<S::Primary>)>,
    mut commands: Commands,
    mut replay_control: EventWriter<MapLayerActionEvent<S::Primary>>,
    system_config: Local<CreateShardState<S>>,
) where
    S: MapShard,
{
    for (root_entity, chunk_root) in new_root_query.iter() {
        log::debug!(
            "Chunk [{:?}]: Create {} layer",
            chunk_root.id,
            simple_type_name::<S::Tile>()
        );

        let layer_entity = system_config.spawn(&mut commands, root_entity, &layer_config).id();
        layer_tracker.track(chunk_root.id, layer_entity);
        replay_control.write(MapLayerActionEvent::Track(chunk_root.id));
    }
}

/// When a chunk is despawned, perform some cleanup.
pub fn remove_shard<S>(
    mut layer_tracker: ResMut<MapLayerTracker<S::Primary>>,
    mut removed_component: RemovedComponents<S::Primary>,
    mut replay_control: EventWriter<MapLayerActionEvent<S::Primary>>,
) where
    S: MapShard,
{
    for entity in removed_component.read() {
        if let Some(chunk_id) = layer_tracker.untrack(&entity) {
            log::debug!("Chunk [{chunk_id:?}]: Remove {} layer", simple_type_name::<S::Tile>());
            // Notify the replay system to untrack this layer
            replay_control.write(MapLayerActionEvent::Untrack(chunk_id));
        }
    }
}

/// Local configuration and state for the `process_shard_notification_events` system
pub struct ProcessNotificationEventState<S>
where
    S: MapShard,
{
    snapshot_frequency: usize,
    next_snapshot: usize,
    _ph: PhantomData<S>,
}

impl<S> FromWorld for ProcessNotificationEventState<S>
where
    S: MapShard,
{
    fn from_world(world: &mut World) -> Self {
        let config = world.get_resource::<MapShardSystemConfig<S>>().unwrap_or_else(|| {
            panic!(
                "MapShardSystemConfig not found for {} layer",
                simple_type_name::<S::Tile>()
            )
        });
        let mut state = Self {
            snapshot_frequency: config.snapshot_frequency,
            next_snapshot: usize::MAX,
            _ph: PhantomData,
        };
        state.update_next_snapshot(0);
        state
    }
}

impl<S> ProcessNotificationEventState<S>
where
    S: MapShard,
{
    fn should_snapshot(&self, version: usize) -> bool {
        self.snapshot_frequency > 0 && version >= self.next_snapshot
    }

    fn update_next_snapshot(&mut self, current_version: usize) {
        if self.snapshot_frequency > 0 {
            self.next_snapshot = current_version + self.snapshot_frequency;
        } else {
            self.next_snapshot = usize::MAX;
        }
    }
}

/// Process 'MapLayerNotificationEvent' events.
pub fn process_shard_notification_events<S>(
    layer_config: Res<S::Config>,
    layer_tracker: ResMut<MapLayerTracker<S::Primary>>,
    mut layers: Query<(&mut MapLayerInfo<S::Primary>, &mut S::Primary, Option<&mut S::Audit>)>,
    mut sync_events: EventReader<MapLayerNotificationEvent<S::Primary>>,
    mut control_events: EventWriter<MapLayerActionEvent<S::Primary>>,
    mut system_config: Local<ProcessNotificationEventState<S>>,
) where
    S: MapShard,
{
    for event in sync_events.read() {
        match event {
            MapLayerNotificationEvent::Initial { id } => {
                log::debug!("Chunk [{id:?}]: Empty layer");
                if let Some((mut info, mut layer, mut layer_change)) =
                    layer_tracker.get_entity(*id).and_then(|e| layers.get_mut(e).ok())
                {
                    log::debug!("Chunk [{id:?}]: Initializing layer from empty state");
                    info.version = MapLayerVersion::new();
                    info.checksum = MapLayerChecksum::new();
                    if let Err(err) = layer.load_from_empty(&layer_config, layer_change.as_deref_mut()) {
                        log::error!("Chunk [{id:?}]: Failed to load empty layer data: {err}");
                        layer.clear();
                        info.version = MapLayerVersion::new();
                        info.checksum = MapLayerChecksum::new();
                        if let Some(change) = layer_change.as_deref_mut() {
                            change.clear();
                        }
                    } else {
                        system_config.update_next_snapshot(0);
                    }
                } else {
                    log::warn!("Chunk [{id:?}]: Received initial notification for unknown layer");
                }
            }
            MapLayerNotificationEvent::Snapshot {
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
                } else {
                    log::warn!("Chunk [{id:?}]: Received snapshot notification for unknown layer");
                }
            }
            MapLayerNotificationEvent::Update {
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

                        let snapshot = if system_config.should_snapshot(info.version.0) {
                            log::debug!(
                                "Chunk [{id:?}]: Create snapshot for {} layer with checksum={:?}",
                                simple_type_name::<S::Tile>(),
                                info.checksum
                            );
                            system_config.update_next_snapshot(info.version.0);
                            match layer.save_to_bytes(&layer_config) {
                                Ok(data) => Some(data),
                                Err(err) => {
                                    log::error!(
                                        "Chunk [{id:?}]: Failed to create snapshot for {} layer: {err}",
                                        simple_type_name::<S::Tile>()
                                    );
                                    None
                                }
                            }
                        } else {
                            None
                        };

                        control_events.write(MapLayerActionEvent::Snapshot {
                            id: *id,
                            version: info.version,
                            checksum: info.checksum,
                            snapshot,
                        });
                    } else {
                        log::warn!(
                            "Chunk [{id:?}]: Ignored out-of-order operation for {} layer (current version={:?}, operation version={:?})",
                            simple_type_name::<S::Tile>(),
                            info.version,
                            *evt_version
                        );
                    }
                } else {
                    log::warn!("Chunk [{id:?}]: Received update notification for unknown layer");
                }
            }
        }
    }
}
