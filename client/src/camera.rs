use crate::{avatar::Avatar, HUD_LAYER};
use bevy::{
    app::{App, Plugin, PreUpdate, Startup},
    core_pipeline::core_3d::Camera3d,
    ecs::{
        entity::Entity,
        error::BevyError,
        name::Name,
        query::With,
        system::{Commands, Query},
    },
    input::keyboard::KeyCode,
    math::{Quat, Vec3},
    render::{camera::Camera, view::NoIndirectDrawing},
    transform::components::Transform,
};
use shine_game::{
    app::{AppGameSchedule, CameraSimulate},
    camera_rig::{rigs, CameraRig, CameraRigPlugin, DebugCameraTarget},
    input_manager::{ActionState, InputManagerPlugin, InputMap, KeyboardInput},
};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CameraRigPlugin { enable_debug: true });
        app.add_plugins(InputManagerPlugin::<CameraAction>::default());

        app.add_systems(Startup, spawn_camera);

        app.add_systems(PreUpdate, toggle_camera_debug);
        app.add_update_systems(CameraSimulate::PreparePose, follow_avatar);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CameraAction {
    Debug,
}

fn spawn_camera(mut commands: Commands) -> Result<(), BevyError> {
    let camera = {
        let mut rig = CameraRig::new()
            .with(rigs::Position::new(Vec3::ZERO))
            .with(rigs::Rotation::new(Quat::default()))
            .with(rigs::Predict::position(1.25))
            .with(rigs::Arm::new(Vec3::new(0.0, 3.5, -5.5)))
            .with(rigs::Predict::position(2.5))
            .with(rigs::LookAt::new(Vec3::Y).duration(1.25).predictive(true));

        let input_map = InputMap::new().with_binding(CameraAction::Debug, KeyboardInput::new(KeyCode::F12))?;

        (
            Name::new("Main camera"),
            Camera3d::default(),
            NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
            rig.calculate_transform(0.0, None),
            rig,
            input_map,
        )
    };
    commands.spawn(camera);

    Ok(())
}

fn toggle_camera_debug(
    camera_q: Query<(Entity, &ActionState<CameraAction>, Option<&DebugCameraTarget>), With<Camera>>,
    mut commands: Commands,
) {
    for (entity, action, debug_target) in camera_q.iter() {
        if action.just_pressed(&CameraAction::Debug) {
            if debug_target.is_some() {
                commands.entity(entity).remove::<DebugCameraTarget>();
            } else {
                commands.entity(entity).insert(DebugCameraTarget {
                    watermark_layer: Some(HUD_LAYER),
                });
            }
        }
    }
}

fn follow_avatar(
    avatar_q: Query<&Transform, With<Avatar>>,
    mut camera_q: Query<&mut CameraRig, With<Camera>>,
) -> Result<(), BevyError> {
    let avatar = avatar_q.single()?;
    let mut camera_rig = camera_q.single_mut()?;

    camera_rig.driver_mut::<rigs::Position>().position = avatar.translation;
    camera_rig.driver_mut::<rigs::Rotation>().rotation = avatar.rotation;
    camera_rig.driver_mut::<rigs::LookAt>().target = avatar.translation + Vec3::Y;

    Ok(())
}
