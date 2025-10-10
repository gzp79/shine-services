use bevy::{
    app::{App, Startup, Update},
    asset::Assets,
    camera::{Camera, Camera2d},
    color::{palettes::css, Color},
    ecs::{
        component::Component,
        error::BevyError,
        name::Name,
        system::{Commands, Query, ResMut},
    },
    gizmos::gizmos::Gizmos,
    input::keyboard::KeyCode,
    math::{
        primitives::{
            Annulus, Capsule2d, Circle, CircularSector, CircularSegment, Ellipse, Rectangle, RegularPolygon, Rhombus,
        },
        Isometry2d, Mat2, Quat, Vec2, Vec3,
    },
    mesh::{Mesh, Mesh2d},
    render::view::NoIndirectDrawing,
    sprite_render::{ColorMaterial, MeshMaterial2d},
    tasks::BoxedFuture,
    transform::components::{GlobalTransform, Transform},
    ui::{widget::Text, Node, PositionType, Val},
    utils::default,
};
use shine_game::{
    app::{init_application, platform, GameSetup, PlatformInit},
    input_manager::{ActionState, InputManagerPlugin, InputMap, KeyboardInput, PinchData, TwoFingerGesture},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    ToggleHelp,
    SwitchMode,
    ResetCamera,
    Debug,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use shine_game::app::platform::{start_game, Config};

    init_application(GameExample);
    start_game(Config::default());
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    init_application(GameExample);
}

#[derive(Component)]
struct AppState {
    show_help: bool,
    use_start_value: bool,
    start_matrix: Option<Transform>,
}

struct GameExample;

impl GameSetup for GameExample {
    type GameConfig = ();

    fn create_setup(&self, _config: &platform::Config) -> BoxedFuture<'static, Self::GameConfig> {
        Box::pin(async move {})
    }

    fn setup_application(&self, app: &mut App, config: &platform::Config, _game_config: ()) {
        app.platform_init(config);

        app.add_plugins((
            //InputManagerConfigurePlugin::default().with_emulate_pinch_gesture(true)
            InputManagerPlugin::<Action>::default(),
        ));

        app.add_systems(Startup, setup)
            .add_systems(Update, (handle_control, update_camera_world_pos, show_status));
    }
}

fn setup(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    commands.spawn((
        Camera2d,
        Camera { ..default() },
        NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
    ));

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
        .with_binding(Action::ResetCamera, KeyboardInput::new(KeyCode::Backspace))?
        .with_binding(Action::ToggleHelp, KeyboardInput::new(KeyCode::F1))?
        .with_binding(Action::SwitchMode, KeyboardInput::new(KeyCode::F2))?
        .with_binding(Action::ResetCamera, KeyboardInput::new(KeyCode::F5))?
        .with_binding(Action::Debug, KeyboardInput::new(KeyCode::F9))?;
    let mode = AppState {
        show_help: true,
        use_start_value: true,
        start_matrix: None,
    };

    commands.spawn((
        Name::new("Input control"),
        input_map,
        TwoFingerGesture::new(),
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
    mut camera_q: Query<(&Camera, &GlobalTransform, &mut Transform)>,
) -> Result<(), BevyError> {
    let (action, mut app_state) = actions_q.single_mut()?;
    let (camera, gt, mut camera_transform) = camera_q.single_mut()?;

    if action.just_pressed(&Action::ResetCamera) {
        *camera_transform = Transform::IDENTITY;
    }

    if action.just_pressed(&Action::ToggleHelp) {
        app_state.show_help = !app_state.show_help;
    }

    if action.just_pressed(&Action::SwitchMode) {
        app_state.use_start_value = !app_state.use_start_value;
        if !app_state.use_start_value {
            app_state.start_matrix = None;
        }
    }

    if action.just_pressed(&Action::Debug) {
        // perform a manual gesture emulation to see if it works

        /*let p1 = Vec2::new(100., 100.);
        let p2 = Vec2::new(150., 150.);
        let q1 = Vec2::new(75., 75.);
        let q2 = Vec2::new(175., 175.);*/

        let p1 = Vec2::new(100., 100.);
        let p2 = Vec2::new(150., 150.);
        let q1 = Vec2::new(210., 21.);
        let q2 = Vec2::new(40., 105.);

        log::info!("p1 {p1:?} p2 {p2:?}");
        log::info!("q1 {q1:?} q2 {q2:?}");

        log::info!("global_transform {gt:?}");
        log::info!("camera_transform {camera_transform:?}");
        log::info!("Clip_from_view {:?}", camera.clip_from_view());
        log::info!("inv Clip_from_view {:?}", camera.clip_from_view().inverse());
        log::info!("viewport_rect {:?}", camera.logical_viewport_rect());

        let w1 = camera.viewport_to_world_2d(gt, p1)?;
        let w2 = camera.viewport_to_world_2d(gt, p2)?;

        log::info!("w1 {w1:?} w2 {w2:?}");

        let screen = PinchData {
            start: (p1, p2),
            prev: (p1, p2),
            current: (q1, q2),
        }
        .viewport_to_screen_centered(camera)
        .ok_or("Failed to convert viewport to screen coordinates")?;

        let from_start = true; // if true, use the start position, otherwise use the previous position
        let s = screen.zoom(from_start);
        let phi = screen.rotate(from_start);
        let t = {
            let rot = Mat2::from_angle(phi) * s;
            let p1 = if from_start { screen.start.1 } else { screen.prev.1 };
            screen.current.1 - rot * p1
        };

        log::info!("s {s:?} phi {phi:?} t {t:?}");

        let (s, phi, t) = {
            let inv_s = 1.0 / s;
            let inv_phi = -phi;
            let inv_rot = Mat2::from_angle(inv_phi) * inv_s;
            let inv_t = -(inv_rot * t);
            (inv_s, inv_phi, inv_t)
        };

        log::info!("inv s {s:?} inv phi {phi:?} inv t {t:?}");

        let delta = Transform {
            translation: t.extend(0.0),
            rotation: Quat::from_rotation_z(phi),
            scale: Vec3::splat(s),
        };

        let new_gt = delta * *gt;

        let w1_ = camera.viewport_to_world_2d(&new_gt, q1)?;
        let w2_ = camera.viewport_to_world_2d(&new_gt, q2)?;
        log::info!("w1_ {w1_:?} w2_ {w2_:?}");

        let q1_ = camera.world_to_viewport(&new_gt, w1.extend(0.0));
        let q2_ = camera.world_to_viewport(&new_gt, w2.extend(0.0));

        log::info!("q1_ {q1_:?} q2_ {q2_:?}");
    }

    Ok(())
}

fn update_camera_world_pos(
    mut player_q: Query<(&TwoFingerGesture, &mut AppState)>,
    mut camera_q: Query<(&Camera, &mut Transform)>,
) -> Result<(), BevyError> {
    let (gesture, mut app_state) = player_q.single_mut()?;

    let (camera, mut camera_transform) = camera_q.single_mut()?;

    #[allow(clippy::collapsible_else_if)]
    if !app_state.use_start_value {
        if let Some(view) = gesture.transform_view_2d(camera, &camera_transform, false) {
            *camera_transform = view;
        }
    } else {
        if let Some(start_matrix) = app_state.start_matrix.as_ref() {
            if let Some(view) = gesture.transform_view_2d(camera, start_matrix, true) {
                *camera_transform = view;
            } else {
                app_state.start_matrix = None;
            }
        } else if gesture.screen_data().is_some() {
            log::info!("Pinch gesture started, saving start matrix");
            app_state.start_matrix = Some(*camera_transform);
        }
    }

    Ok(())
}

fn show_status(
    mut players: Query<(&AppState, &ActionState<Action>, &TwoFingerGesture, &mut Text)>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut gizmo: Gizmos,
) -> Result<(), BevyError> {
    let (camera, camera_transform) = camera.single()?;

    for (app_state, _action_state, gesture, mut text) in players.iter_mut() {
        let mut logs = Vec::new();

        if app_state.show_help {
            logs.push("F1 - toggle help".to_string());
            logs.push("F2 - toggle mode".to_string());
            logs.push("F5 - reset camera".to_string());
            //logs.push(format!("F9 - debug"));
            logs.push("".to_string());
        }

        logs.push(format!("Using start value: {:?}", app_state.use_start_value));

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

        if let Some(screen_data) = gesture
            .screen_data()
            .and_then(|data| data.viewport_to_screen_centered(camera))
        {
            logs.push(format!("NDC Pan: {:?}", screen_data.pan(true)));
            logs.push(format!("NDC Zoom: {}", screen_data.zoom(true)));
            logs.push(format!("NDC Rotate: {}", screen_data.rotate(true)));
        } else {
            logs.push("NDC Pan: None".to_string());
            logs.push("NDC Zoom: None".to_string());
            logs.push("NDC Rotate: None".to_string());
        };

        gizmo.rect_2d(Isometry2d::IDENTITY, Vec2::new(500.0, 500.0), css::BLUE);
        gizmo.rect_2d(Isometry2d::IDENTITY, Vec2::new(100.0, 100.0), css::GREEN);
        gizmo.rect_2d(Isometry2d::IDENTITY, Vec2::new(20.0, 20.0), css::RED);
        gizmo.line_2d(Vec2::ZERO, Vec2::X * 100., css::RED);
        gizmo.line_2d(Vec2::ZERO, Vec2::Y * 100., css::GREEN);

        text.0 = logs.join("\n");
    }
    Ok(())
}
