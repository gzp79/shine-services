use crate::HUD_LAYER;
use bevy::{core_pipeline::core_2d::Camera2d, ecs::system::Commands, render::camera::Camera};

pub fn spawn_hud_camera(mut commands: Commands) {
    let debug_2d_camera = (
        Camera2d,
        Camera {
            order: 100,
            ..Default::default()
        },
        HUD_LAYER,
    );
    commands.spawn(debug_2d_camera);
}
