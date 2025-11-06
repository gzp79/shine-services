use crate::hud::{hud_camera::spawn_hud_camera, hud_gizmo::HUDGizmosConfig};
use bevy::{
    app::{App, Plugin, Startup},
    camera::visibility::RenderLayers,
    gizmos::{config::GizmoConfig, AppGizmoBuilder},
};

pub const HUD_LAYER: RenderLayers = RenderLayers::layer(31);

pub struct HUDPlugin;

impl Plugin for HUDPlugin {
    fn build(&self, app: &mut App) {
        app.insert_gizmo_config(
            HUDGizmosConfig::default(),
            GizmoConfig {
                render_layers: HUD_LAYER,
                ..Default::default()
            },
        );

        app.add_systems(Startup, spawn_hud_camera);
    }
}
