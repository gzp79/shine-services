use crate::map::{
    MapAuditedLayer, MapChunk, MapChunkId, MapLayer, MapLayerActionMessage, MapLayerChecksum, MapLayerClientChannels,
    MapLayerConfig, MapLayerIO, MapLayerIOExt, MapLayerInfo, MapLayerNotificationMessage, MapLayerOf, MapLayerTracker,
    MapLayerVersion, Tile,
};
use bevy::{
    ecs::{
        entity::Entity,
        error::BevyError,
        lifecycle::{Add, Remove},
        message::{MessageReader, MessageWriter},
        name::Name,
        observer::On,
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
        chunk_id: MapChunkId,
        layer_config: &S::Config,
    ) -> EntityCommands<'c> {
        // spawn the layer as a child of the chunk root
        let layer = {
            let version = MapLayerInfo::<S::Primary>::new();

            let mut layer = S::Primary::new();
            if self.initialize_on_spawn {
                layer.initialize(layer_config);
            }

            (
                Name::new(format!(
                    "{}({},{})",
                    simple_type_name::<S::Tile>(),
                    chunk_id.0,
                    chunk_id.1
                )),
                version,
                layer,
                MapLayerOf(chunk_root),
            )
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

/// Spawn layer components and performs some book-keeping when a new MapChunk is spawned.
/// MapChunk component is added only once during the root entity spawn, so this system runs only once per chunk.
#[allow(clippy::type_complexity)]
pub fn create_shard<S>(
    new_chunk_trigger: On<Add, MapChunk>,
    layer_config: Res<S::Config>,
    mut layer_tracker: ResMut<MapLayerTracker<S::Primary>>,
    chunk_root_q: Query<&MapChunk>,
    mut commands: Commands,
    mut replay_control: MessageWriter<MapLayerActionMessage<S::Primary>>,
    system_config: Local<CreateShardState<S>>,
) -> Result<(), BevyError>
where
    S: MapShard,
{
    let chunk_root_entity = new_chunk_trigger.entity;
    let chunk_root = chunk_root_q.get(chunk_root_entity)?;
    log::debug!(
        "Chunk [{:?}]: Create {} layer",
        chunk_root.id,
        simple_type_name::<S::Tile>()
    );

    let layer_entity = system_config
        .spawn(&mut commands, chunk_root_entity, chunk_root.id, &layer_config)
        .id();
    layer_tracker.track(chunk_root.id, layer_entity);

    // Notify the replay system to start tracking this layer
    replay_control.write(MapLayerActionMessage::Track(chunk_root.id));
    Ok(())
}

/// When the (primary) map layer is removed, perform some cleanup.
/// The layer component is removed only once when the entity is despawned.
pub fn remove_shard<S>(
    removed_layer_trigger: On<Remove, S::Primary>,
    mut layer_tracker: ResMut<MapLayerTracker<S::Primary>>,
    mut replay_control: MessageWriter<MapLayerActionMessage<S::Primary>>,
) -> Result<(), BevyError>
where
    S: MapShard,
{
    let chunk_id = layer_tracker.untrack(&removed_layer_trigger.entity).ok_or_else(|| {
        BevyError::from(format!(
            "Failed to untrack {} layer for entity {:?}",
            simple_type_name::<S::Tile>(),
            removed_layer_trigger.entity
        ))
    })?;

    log::debug!("Chunk [{chunk_id:?}]: Remove {} layer", simple_type_name::<S::Tile>());

    // Notify the replay system to untrack this layer
    replay_control.write(MapLayerActionMessage::Untrack(chunk_id));
    Ok(())
}

/// Local configuration and state for the `process_shard_notification_messages` system
pub struct ProcessNotificationMessageState<S>
where
    S: MapShard,
{
    snapshot_frequency: usize,
    next_snapshot: usize,
    _ph: PhantomData<S>,
}

impl<S> FromWorld for ProcessNotificationMessageState<S>
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

impl<S> ProcessNotificationMessageState<S>
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

/// Process 'MapLayerNotificationMessage' messages.
pub fn process_shard_notification_messages<S>(
    layer_config: Res<S::Config>,
    layer_tracker: ResMut<MapLayerTracker<S::Primary>>,
    mut layers: Query<(&mut MapLayerInfo<S::Primary>, &mut S::Primary, Option<&mut S::Audit>)>,
    mut sync_messages: MessageReader<MapLayerNotificationMessage<S::Primary>>,
    mut action_messages: MessageWriter<MapLayerActionMessage<S::Primary>>,
    mut system_config: Local<ProcessNotificationMessageState<S>>,
) where
    S: MapShard,
{
    for message in sync_messages.read() {
        match message {
            MapLayerNotificationMessage::Initial { id } => {
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
            MapLayerNotificationMessage::Snapshot {
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
            MapLayerNotificationMessage::Update {
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

                        action_messages.write(MapLayerActionMessage::Snapshot {
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
