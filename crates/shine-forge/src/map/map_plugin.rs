use bevy::app::{App, Plugin};

#[derive(Default)]
pub struct MapPlugin {}

impl Plugin for MapPlugin {
    fn build(&self, _app: &mut App) {}
}
