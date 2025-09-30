use bevy::ecs::resource::Resource;

/// Global world configuration settings.
#[derive(Resource)]
pub struct WorldMapConfig {
    pub ground_tile_size: f32,
}

impl Default for WorldMapConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldMapConfig {
    pub fn new() -> Self {
        Self { ground_tile_size: 10.0 }
    }
}
