use bevy::ecs::resource::Resource;

/// Configuration for the map that applies to all chunks
#[derive(Resource, Clone, Debug)]
pub struct MapConfig {
    pub width: usize,
    pub height: usize,
}

impl MapConfig {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }
}
