use crate::{sentinel::Sentinel, GameState};
use bevy::{
    core_pipeline::core_3d::Camera3d,
    ecs::{
        component::Component,
        error::BevyError,
        query::{With, Without},
        system::{Commands, Query, Res},
    },
    math::Vec3,
    render::view::NoIndirectDrawing,
    state::state_scoped::StateScoped,
    time::Time,
    transform::components::Transform,
};
use shine_game::camera_rig::{rigs, CameraRig};

#[derive(Component)]
pub struct MainCamera;

pub fn spawn(sentinel_q: Query<&Transform, With<Sentinel>>, mut commands: Commands) -> Result<(), BevyError> {
    let sentinel_transform = sentinel_q.single()?;
    let target_pos = sentinel_transform.translation + Vec3::Y;
    //let look_pos = target_pos + Vec3::Y * look_distance;

    let rig = CameraRig::builder()
        .with(rigs::Position::new(target_pos))
        .with(rigs::Rotation::new(sentinel_transform.rotation))
        //.with(rigs::Smooth::new_position(1.25).predictive(false))
        //.with(rigs::Arm::new(Vec3::new(0.0, 3.5, -5.5)))
        //.with(Smooth::new_position(2.5).predictive(true))
        .with(rigs::LookAt::new(Vec3::ZERO).smoothness(1.25).predictive(false))
        .build();

    let camera = (
        MainCamera,
        StateScoped(GameState::InWorld),
        Camera3d::default(),
        NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
        *rig.transform(),
        rig,
    );

    commands.spawn(camera);
    Ok(())
}

pub fn follow_sentinel(
    sentinel_q: Query<&Transform, With<Sentinel>>,
    mut camera_q: Query<(&mut CameraRig, &mut Transform), (With<MainCamera>, Without<Sentinel>)>,
    time: Res<Time>,
) -> Result<(), BevyError> {
    let sentinel_transform = sentinel_q.single()?;
    let (mut camera_rig, mut camera_transform) = camera_q.single_mut()?;

    camera_rig.driver_mut::<rigs::Position>().position = sentinel_transform.translation;
    camera_rig.driver_mut::<rigs::Rotation>().rotation = sentinel_transform.rotation;
    //camera_rig.driver_mut::<LookAt>().target = sentinel_transform.translation + Vec3::Y * look_distance;

    *camera_transform = camera_rig.update(time.delta_secs());

    Ok(())
}
