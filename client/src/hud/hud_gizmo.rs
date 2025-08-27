use bevy::{
    gizmos::{config::GizmoConfigGroup, gizmos::Gizmos},
    reflect::Reflect,
};

// We can create our own gizmo config group!
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct HUDGizmosConfig {}

pub type HUDGizmos<'w, 's> = Gizmos<'w, 's, HUDGizmosConfig>;
