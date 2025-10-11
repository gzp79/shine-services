use bevy::ecs::resource::Resource;

/// Global world configuration settings.
#[derive(Resource)]
pub struct WorldConfig {
    pub ground_chunk_size: u32,
    pub ground_tile_size: f32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldConfig {
    pub fn new() -> Self {
        Self {
            ground_chunk_size: 4,
            ground_tile_size: 10.0,
        }
    }
}
