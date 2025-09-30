use bevy::color::palettes::css;
use bevy::prelude::*;

const COLORS: &[Color] = &[
    Color::Srgba(css::DARK_GREEN),
    Color::Srgba(css::DARK_RED),
    Color::Srgba(css::DARK_BLUE),
    Color::Srgba(css::DARK_ORANGE),
    Color::Srgba(css::PURPLE),
    Color::Srgba(css::YELLOW),
    Color::Srgba(css::LIGHT_GREEN),
    Color::Srgba(css::LIGHT_BLUE),
    Color::Srgba(css::ALICE_BLUE),
    Color::Srgba(css::DARK_GRAY),
    Color::Srgba(css::LIGHT_GRAY),
    Color::Srgba(css::WHITE),
];

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_light);
    }
}
#[derive(Component)]
pub struct ChunkRender;

fn spawn_floor(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut c = 0;
    let mesh = Mesh3d(meshes.add(Mesh::from(Plane3d::default().mesh().size(1.0, 1.0))));
    for x in -5..=5 {
        for z in -5..=5 {
            let material = MeshMaterial3d(materials.add(StandardMaterial {
                base_color: COLORS[c % COLORS.len()],
                ..default()
            }));
            let transform = Transform::from_xyz(x as f32, 0.0, z as f32);
            c += 1;
            commands.spawn((mesh.clone(), material, transform, ChunkRender));
        }
    }
}

fn spawn_light(mut commands: Commands) {
    let light = (
        PointLight {
            intensity: 2000.0 * 1000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 0.0),
    );

    commands.spawn(light);
}
