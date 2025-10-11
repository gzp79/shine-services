use crate::HUD_LAYER;
use bevy::{
    camera::{Camera, Camera2d},
    ecs::system::Commands,
};

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
