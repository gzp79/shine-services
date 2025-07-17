use bevy::prelude::*;
use shine_game::{
    application,
    input_manager::{ActionState, InputManagerPlugin, InputMap, KeyboardInput, PinchPan, PinchRotate, PinchZoom},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    PinchPan,
    PinchZoom,
    PinchRotate,

    ToggleMode,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use shine_game::application::{create_application, platform::Config};

    application::init(setup_game);
    let mut app = create_application(Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    application::init(setup_game);
}

#[derive(Component)]
struct Mode {
    is_simple: bool,
}

impl Mode {
    fn title(&self) -> String {
        format!("Mode: {}", if self.is_simple { "simple" } else { "advanced" })
    }
}

fn setup_game(app: &mut App) {
    app.add_plugins(InputManagerPlugin::<Action>::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (toggle_mode, update_camera_simple.run_if(is_simple_mode)));
}

fn setup(
    mut window: Query<&mut Window>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    let mut window = window.single_mut()?;

    commands.spawn((Camera2d, Camera { ..default() }));

    // spawn some content
    {
        let shapes = [
            meshes.add(Circle::new(50.0)),
            meshes.add(CircularSector::new(50.0, 1.0)),
            meshes.add(CircularSegment::new(50.0, 1.25)),
            meshes.add(Ellipse::new(25.0, 50.0)),
            meshes.add(Annulus::new(25.0, 50.0)),
            meshes.add(Capsule2d::new(25.0, 50.0)),
            meshes.add(Rhombus::new(75.0, 100.0)),
            meshes.add(Rectangle::new(50.0, 100.0)),
            meshes.add(RegularPolygon::new(50.0, 6)),
        ];

        const NUM_SHAPES_PER_ROW: usize = 3;
        let num_shapes = shapes.len();

        let mut x = 0;
        let mut y = 0;
        for (i, shape) in shapes.into_iter().enumerate() {
            // Distribute colors evenly across the rainbow.
            let color = Color::hsl(360. * i as f32 / num_shapes as f32, 0.95, 0.7);

            commands.spawn((
                Mesh2d(shape),
                MeshMaterial2d(materials.add(color)),
                Transform::from_xyz(-200. + x as f32 * 100., -200. + y as f32 * 100., 0.0),
            ));

            x += 1;
            if x >= NUM_SHAPES_PER_ROW {
                x = 0;
                y += 1;
            }
        }
    }

    let input_map = InputMap::new()
        .with_dual_axis(Action::PinchPan, PinchPan::delta())
        .with_axis(Action::PinchZoom, PinchZoom::delta())
        .with_axis(Action::PinchRotate, PinchRotate::delta())
        .with_button(Action::ToggleMode, KeyboardInput::new(KeyCode::Space));
    let mode = Mode { is_simple: true };

    window.title = mode.title();
    commands.spawn((Name::new("Input control"), input_map, mode));

    Ok(())
}

fn toggle_mode(
    mut actions_q: Query<(&ActionState<Action>, &mut Mode)>,
    mut window_q: Query<&mut Window>,
) -> Result<(), BevyError> {
    let (action, mut mode) = actions_q.single_mut()?;
    let mut window = window_q.single_mut()?;

    if action.just_pressed(&Action::ToggleMode) {
        mode.is_simple = !mode.is_simple;
        window.title = mode.title();
    }

    Ok(())
}

fn is_simple_mode(camera: Query<&Mode>) -> bool {
    camera.single().map(|m| m.is_simple).unwrap_or_default()
}

fn update_camera_simple(
    actions_q: Query<&ActionState<Action>>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    mut window_q: Query<&mut Window>,
) -> Result<(), BevyError> {
    let action = actions_q.single()?;
    let (mut transform, mut projection) = camera_q.single_mut()?;
    let mut window = window_q.single_mut()?;

    let Projection::Orthographic(projection) = &mut *projection else {
        unreachable!();
    };

    if let Some(pan) = action.try_dual_axis_value(&Action::PinchPan) {
        // pan is given in viewport coordinates
        let ndc_pan = Vec3::new(-pan.x, pan.y, 0.0) * 2.0 * projection.scale;
        let world_pan = transform.compute_matrix().transform_vector3(ndc_pan);
        transform.translation += world_pan;
    }

    if let Some(zoom) = action.try_axis_value(&Action::PinchZoom) {
        projection.scale /= zoom;
    }

    if let Some(rotate) = action.try_axis_value(&Action::PinchRotate) {
        transform.rotate(Quat::from_rotation_z(rotate));
    }

    window.title = format!("Camera: {:?}", projection.scale);

    Ok(())
}
