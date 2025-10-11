use bevy::ecs::resource::Resource;
use tokio::runtime::Runtime;

#[derive(Resource)]
pub struct TokioRuntime {
    pub runtime: Runtime,
}
