use bevy::{
    app::{App, Plugin, Startup},
    asset::Assets,
    color::{palettes::css, Color},
    ecs::{
        component::Component,
        error::{BevyError, Result},
        name::Name,
        system::{Commands, Query, Res, ResMut},
    },
    input::keyboard::KeyCode,
    math::{primitives::Tetrahedron, Quat, Vec3},
    mesh::{Mesh, Mesh3d},
    pbr::{MeshMaterial3d, StandardMaterial},
    time::Time,
    transform::components::Transform,
};
use shine_game::{
    app::{AppGameSchedule, GameSystems},
    input_manager::{ActionState, InputManagerPlugin, InputMap, KeyboardInput, VirtualPad},
};

pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<AvatarAction>::default());

        app.add_systems(Startup, spawn_avatar);
        app.add_update_systems(GameSystems::Action, handle_avatar_input);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AvatarAction {
    Move,
    Rotate,

    Debug1,
    Debug2,
}

#[derive(Component)]
pub struct Avatar;

fn spawn_avatar(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    let avatar = {
        let input_map = InputMap::new()
            .with_binding(AvatarAction::Move, VirtualPad::from_keys(KeyCode::KeyW, KeyCode::KeyS))?
            .with_binding(
                AvatarAction::Rotate,
                VirtualPad::from_keys(KeyCode::KeyA, KeyCode::KeyD),
            )?
            .with_binding(AvatarAction::Debug1, KeyboardInput::new(KeyCode::F1))?
            .with_binding(AvatarAction::Debug2, KeyboardInput::new(KeyCode::F2))?;

        (
            Name::new("Avatar"),
            Mesh3d(meshes.add(Tetrahedron::new(
                Vec3::new(-1.0, -1.0, 0.0),
                Vec3::new(1.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, -1.0, 0.5),
            ))),
            MeshMaterial3d(materials.add(Color::Srgba(css::DARK_BLUE))),
            Transform::IDENTITY,
            Avatar,
            input_map,
        )
    };
    commands.spawn(avatar);

    Ok(())
}

pub fn handle_avatar_input(
    mut avatar_q: Query<(&ActionState<AvatarAction>, &mut Transform)>,
    time: Res<Time>,
) -> Result<(), BevyError> {
    let (actions, mut avatar) = avatar_q.single_mut()?;

    let mov = actions.axis_value(&AvatarAction::Move);
    let mut rot = actions.axis_value(&AvatarAction::Rotate);

    rot *= time.delta_secs() * 2.0;
    avatar.rotation = Quat::from_rotation_y(rot) * avatar.rotation;

    let mut move_vec = avatar.rotation * Vec3::Z * mov;
    move_vec.y = 0.0;
    if move_vec.length_squared() > 0.0 {
        move_vec = move_vec.normalize();
    }
    move_vec *= time.delta_secs() * 5.0;

    avatar.translation += move_vec;

    Ok(())
}
