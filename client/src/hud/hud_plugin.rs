use crate::{
    hud::{hud_camera::spawn_hud_camera, hud_gizmo::HUDGizmosConfig},
    HUD_LAYER,
};
use bevy::{
    app::{App, Plugin, Startup},
    gizmos::{config::GizmoConfig, AppGizmoBuilder},
};

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
