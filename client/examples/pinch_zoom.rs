use bevy::{color::palettes::css, prelude::*, render::view::NoIndirectDrawing};
use shine_game::{
    application,
    input_manager::{ActionState, InputManagerPlugin, InputMap, KeyboardInput, TwoFingerGesture},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    ToggleMode,
    ResetCamera,
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
struct AppState {
    is_simple: bool,
    // during advanced mode, we keep track of the world center that should be fixed relative to the gesture's current center
    world_center: Option<Vec3>,
    start_matrix: Option<Transform>,
}

impl AppState {
    fn title(&self) -> String {
        format!("Mode: {}", if self.is_simple { "simple" } else { "advanced" })
    }
}

fn setup_game(app: &mut App) {
    app.add_plugins(InputManagerPlugin::<Action>::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_control, update_camera_world_pos, show_status));
}

fn setup(
    mut window: Query<&mut Window>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    let mut window = window.single_mut()?;

    let camera = commands
        .spawn((
            Camera2d,
            Camera { ..default() },
            NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
        ))
        .id();

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
                Transform::from_xyz(-100. + x as f32 * 100., -100. + y as f32 * 100., 0.0),
            ));

            x += 1;
            if x >= NUM_SHAPES_PER_ROW {
                x = 0;
                y += 1;
            }
        }
    }

    let input_map = InputMap::new()
        .with_button(Action::ResetCamera, KeyboardInput::new(KeyCode::Backspace))
        .with_button(Action::ToggleMode, KeyboardInput::new(KeyCode::Space));
    let mode = AppState {
        is_simple: true,
        world_center: None,
        start_matrix: None,
    };

    window.title = mode.title();
    commands.spawn((
        Name::new("Input control"),
        input_map,
        TwoFingerGesture::new().with_camera(camera),
        mode,
        Text::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

    Ok(())
}

fn handle_control(
    mut actions_q: Query<(&ActionState<Action>, &mut AppState)>,
    mut camera_q: Query<&mut Transform, With<Camera2d>>,
    mut window_q: Query<&mut Window>,
) -> Result<(), BevyError> {
    let (action, mut mode) = actions_q.single_mut()?;
    let mut camera_transform = camera_q.single_mut()?;
    let mut window = window_q.single_mut()?;

    if action.just_pressed(&Action::ToggleMode) {
        mode.is_simple = !mode.is_simple;
        window.title = mode.title();
    }

    if action.just_pressed(&Action::ResetCamera) {
        *camera_transform = Transform::IDENTITY;
    }

    Ok(())
}

fn update_camera_world_pos(
    mut player_q: Query<(&TwoFingerGesture, &mut AppState)>,
    mut camera_q: Query<&mut Transform, With<Camera2d>>,
) -> Result<(), BevyError> {
    let (gesture, mut app_state) = player_q.single_mut()?;

    if app_state.is_simple {
        return Ok(());
    }

    let mut camera_transform = camera_q.single_mut()?;

    if let (Some(screen_data), Some(world_data)) = (gesture.screen_data(), gesture.world_data()) {
        let _ = app_state.world_center.get_or_insert(world_data.start_ray.origin);
        let start_matrix = app_state.start_matrix.get_or_insert_with(|| camera_transform.clone());

        let zoom = screen_data.zoom(true);
        let rotate = screen_data.rotate(true);
        let pan = world_data.pan(true);
        let center = world_data.center();

        let v = Mat3::new(
            screen_data.start.0.x, screen_data.start.0.x, screen_data.start.0.x,
        )

        /*let pivot = center;
        let dt = Transform::from_translation(-pivot)
            .mul_transform(Transform::from_scale(Vec3::splat(1. / zoom)))
            .mul_transform(Transform::from_rotation(Quat::from_rotation_z(rotate)))
            .mul_transform(Transform::from_translation(pivot));
        *camera_transform = start_matrix.mul_transform(dt);*/

        *camera_transform = start_matrix.mul_transform(Transform::from_translation(-pan));
    } else {
        app_state.world_center = None;
        app_state.start_matrix = None;
    }

    Ok(())
}

// fn update_camera(
//     actions_q: Query<&ActionState<Action>, &TwoFingerGesture>,
//     mut camera_q: Query<(&GlobalTransform, &mut Transform, &mut Projection), With<Camera2d>>,
//     mut window_q: Query<&mut Window>,
// ) -> Result<(), BevyError> {
//     let action = actions_q.single()?;
//     let (global_transform, mut transform, mut projection) = camera_q.single_mut()?;
//     let mut window = window_q.single_mut()?;

//     let Projection::Orthographic(projection) = &mut *projection else {
//         unreachable!();
//     };

//     if let Some(pan) = action.try_dual_axis_value(&Action::PinchPan) {
//         // pan is given in viewport coordinates
//         let view_matrix = global_transform.compute_matrix().inverse();
//         //todo: should it consider the viewport size ? (viewport is in pixel, ndc is in -1..1 range)
//         let ndc_pan = Vec3::new(-pan.x, pan.y, 0.0) * projection.scale;
//         let world_pan = view_matrix.transform_vector3(ndc_pan);
//         transform.translation += world_pan;
//     }

//     if let Some(zoom) = action.try_axis_value(&Action::PinchZoom) {
//         projection.scale /= zoom;
//     }

//     if let Some(rotate) = action.try_axis_value(&Action::PinchRotate) {
//         transform.rotate(Quat::from_rotation_z(rotate));
//     }

//     window.title = format!("Camera: {:?}", projection.scale);

//     Ok(())
// }

fn show_status(
    mut players: Query<(&ActionState<Action>, &TwoFingerGesture, &AppState, &mut Text)>,
    camera: Query<(&Camera, &GlobalTransform, &Projection)>,
    mut gizmo: Gizmos,
) -> Result<(), BevyError> {
    let (camera, camera_transform, projection) = camera.single()?;

    for (_action_state, gesture, app_state, mut text) in players.iter_mut() {
        let mut logs = Vec::new();

        //logs.push(format!("Pan: {projection:?}"));

        logs.push(if app_state.is_simple {
            "Mode: Simple".to_string()
        } else {
            "Mode: Advanced".to_string()
        });

        if let Some(screen_data) = gesture.screen_data() {
            logs.push(format!("Pan: {:?}", screen_data.pan(true)));
            logs.push(format!("Zoom: {}", screen_data.zoom(true)));
            logs.push(format!("Rotate: {}", screen_data.rotate(true)));

            // show touch points
            if let Ok(p0) = camera.viewport_to_world_2d(camera_transform, screen_data.current.0) {
                gizmo.circle_2d(Isometry2d::from_translation(p0), 10., css::GRAY);
            }
            if let Ok(p1) = camera.viewport_to_world_2d(camera_transform, screen_data.current.1) {
                gizmo.circle_2d(Isometry2d::from_translation(p1), 10., css::GRAY);
            }
        } else {
            logs.push("Pan: None".to_string());
            logs.push("Zoom: None".to_string());
            logs.push("Rotate: None".to_string());
        };

        if let Some(world_data) = gesture.world_data() {
            logs.push("World Center: Active".to_string());
            let start_center = world_data.start_ray.origin;
            let prev_center = world_data.prev_ray.origin;
            let current_center = world_data.current_ray.origin;

            gizmo.circle(Isometry3d::from_translation(start_center), 25., css::RED);
            gizmo.circle(Isometry3d::from_translation(prev_center), 10., css::GREEN);
            gizmo.circle(Isometry3d::from_translation(current_center), 4., css::BLUE);
        } else {
            logs.push("World Center: None".to_string());
        };

        if let Some(center) = app_state.world_center {
            gizmo.circle(Isometry3d::from_translation(center), 15.0, css::YELLOW);
            logs.push(format!("Gesture world center: {:.2},{:.2}", center.x, center.y));
        } else {
            logs.push("Gesture world center: None".to_string());
        }

        gizmo.rect_2d(Isometry2d::IDENTITY, Vec2::new(500.0, 500.0), css::BLUE);
        gizmo.rect_2d(Isometry2d::IDENTITY, Vec2::new(100.0, 100.0), css::GREEN);
        gizmo.rect_2d(Isometry2d::IDENTITY, Vec2::new(20.0, 20.0), css::RED);
        gizmo.line_2d(Vec2::ZERO, Vec2::X * 100., css::RED);
        gizmo.line_2d(Vec2::ZERO, Vec2::Y * 100., css::GREEN);

        text.0 = logs.join("\n");
    }
    Ok(())
}
