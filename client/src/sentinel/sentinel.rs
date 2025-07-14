use bevy::{
    asset::Assets,
    color::{palettes::css, Color},
    ecs::{
        component::Component,
        entity::Entity,
        error::BevyError,
        query::With,
        resource::Resource,
        system::{Commands, Query, Res, ResMut},
    },
    input::keyboard::KeyCode,
    math::{primitives, Quat, Vec3},
    pbr::{MeshMaterial3d, StandardMaterial},
    render::{
        mesh::{Mesh, Mesh3d},
        view::Visibility,
    },
    time::Time,
    transform::components::Transform,
    window::Window,
};
use shine_game::{
    application::WindowExt,
    input_manager::{
        ActionState, EdgeSize, InputMap, KeyboardInput, MousePositionInput, ScreenEdgeScroll, TouchPositionInput,
        VirtualDPad, VirtualPad,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SentinelAction {
    Move,
    Rotate,

    ToggleShowSentinel,
    ToggleFreeView,
}

#[derive(Resource)]
pub struct SentinelConfig {
    pub speed: f32,
}

/// The entity that represents the world spectator in the game.
#[derive(Component)]
pub struct Sentinel;

pub fn spawn_sentinel(mut windows: Query<&mut Window>, mut commands: Commands) {
    let mut window = windows.single_mut().unwrap();
    window.start_grab(true);

    let input_map = InputMap::<SentinelAction>::new()
        .with_dual_axis(SentinelAction::Move, VirtualDPad::wasd())
        .with_axis(SentinelAction::Rotate, VirtualPad::qe())
        .with_dual_axis(
            SentinelAction::Move,
            ScreenEdgeScroll::new(TouchPositionInput::new(), EdgeSize::Fixed(50.0)),
        )
        .with_dual_axis(
            SentinelAction::Move,
            ScreenEdgeScroll::new(MousePositionInput::new(), EdgeSize::Fixed(50.0)),
        )
        .with_button(SentinelAction::ToggleShowSentinel, KeyboardInput::new(KeyCode::F3))
        .with_button(SentinelAction::ToggleFreeView, KeyboardInput::new(KeyCode::F4));

    let sentinel = (Sentinel, Transform::IDENTITY, input_map);
    commands.spawn(sentinel);
}

pub fn despawn_sentinel(sentinel_q: Query<Entity, With<Sentinel>>, mut commands: Commands) -> Result<(), BevyError> {
    let sentinel = sentinel_q.single()?;
    commands.entity(sentinel).despawn();
    Ok(())
}

pub fn spawn_sentinel_debug(
    sentinel_q: Query<Entity, With<Sentinel>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    let sentinel = sentinel_q.single()?;

    let scl = 0.5;
    let debug = (
        Mesh3d(meshes.add(primitives::Tetrahedron::new(
            Vec3::new(-1.0, 0.0, -1.0) * scl,
            Vec3::new(1.0, 0.0, -1.0) * scl,
            Vec3::new(0.0, 0.0, 1.0) * scl,
            Vec3::new(0.0, 0.5, -1.0) * scl,
        ))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_BLUE))),
        Visibility::Visible,
    );

    commands.entity(sentinel).insert(debug);
    Ok(())
}

pub fn update_debug(
    mut sentinel_q: Query<(&ActionState<SentinelAction>, &mut Visibility), With<Sentinel>>,
) -> Result<(), BevyError> {
    let (action_state, mut visibility) = sentinel_q.single_mut()?;
    if action_state.button(&SentinelAction::ToggleShowSentinel).just_pressed() {
        *visibility = match *visibility {
            Visibility::Visible | Visibility::Inherited => Visibility::Hidden,
            Visibility::Hidden => Visibility::Inherited,
        };
        log::info!("Toggling sentinel visibility, {:?}", visibility);
    }
    Ok(())
}

pub fn move_sentinel(
    mut sentinel_q: Query<(&ActionState<SentinelAction>, &mut Transform), With<Sentinel>>,
    sentinel_config: Res<SentinelConfig>,
    time: Res<Time>,
) -> Result<(), BevyError> {
    let (action_state, mut transform) = sentinel_q.single_mut()?;
    let movement = action_state.dual_axis(&SentinelAction::Move);
    let rotation = action_state.axis(&SentinelAction::Rotate);

    let mut rot = rotation.value;
    rot *= time.delta_secs() * 2.0;
    transform.rotation = Quat::from_rotation_y(rot) * transform.rotation;

    let mut move_vec = Vec3 {
        x: movement.value.x,
        y: 0.0,
        z: movement.value.y,
    };
    move_vec = transform.rotation * move_vec;
    move_vec.y = 0.0;
    move_vec = move_vec.normalize_or_zero();
    move_vec *= time.delta_secs() * sentinel_config.speed;

    transform.translation += move_vec;

    Ok(())
}
