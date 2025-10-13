use bevy::{
    app::{App, Startup, Update},
    camera::{Camera, Camera2d},
    color::Color,
    ecs::{
        component::Component,
        error::BevyError,
        query::With,
        system::{Commands, Local, Query, Res},
    },
    gizmos::gizmos::Gizmos,
    input::{keyboard::KeyCode, mouse::MouseButton},
    math::{Vec2, Vec3},
    tasks::BoxedFuture,
    time::Time,
    ui::{widget::Text, Node, PositionType, Val},
    utils::default,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window},
};
use core::f32;
use shine_game::{
    app::{init_application, platform, GameSetup, PlatformInit},
    input_manager::{
        ActionState, AttachedToGestureSet, DualAxisChord, GestureInput, GestureSet, InputManagerPlugin, InputMap,
        KeyboardInput, MouseButtonInput, MousePosition, TouchPosition, UnistrokeGesture,
    },
    math::{
        jackknife::{GestureId, JackknifeConfig, JackknifePointMath, JackknifeTemplateSet},
        unistroke_templates,
    },
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    MouseClickPosition,
    CtrlMousePosition,
    TouchPosition,

    GestureCircle,
    GestureTriangle,
    GestureRectangle,

    Grab,
}

#[derive(Component)]
struct StatusText;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn main() {
    use shine_game::app::platform::{start_game, Config};

    init_application(GameExample);
    start_game(Config::default());
}

#[cfg(target_family = "wasm")]
pub fn main() {
    init_application(GameExample);
}

const GESTURES: &[(&str, &[Vec2], GestureId)] = &[
    ("Line 0", unistroke_templates::LINE_0, GestureId(0)),
    ("Line 45", unistroke_templates::LINE_45, GestureId(1)),
    ("Line 90", unistroke_templates::LINE_90, GestureId(2)),
    ("Line 135", unistroke_templates::LINE_135, GestureId(3)),
    ("Line 180", unistroke_templates::LINE_180, GestureId(4)),
    ("Line 225", unistroke_templates::LINE_225, GestureId(5)),
    ("Line 270", unistroke_templates::LINE_270, GestureId(6)),
    ("Line 315", unistroke_templates::LINE_315, GestureId(7)),
    ("V", unistroke_templates::V, GestureId(8)),
    ("Triangle", unistroke_templates::TRIANGLE, GestureId(9)),
    ("Rectangle", unistroke_templates::RECTANGLE, GestureId(10)),
    ("Circle", unistroke_templates::CIRCLE, GestureId(11)),
    ("Zig Zag", unistroke_templates::ZIG_ZAG, GestureId(12)),
];

struct GameExample;

impl GameSetup for GameExample {
    type GameConfig = ();

    fn create_setup(&self, _config: &platform::Config) -> BoxedFuture<'static, Self::GameConfig> {
        Box::pin(async move {})
    }

    fn setup_application(&self, app: &mut App, config: &platform::Config, _game_config: ()) {
        app.platform_init(config);

        app.add_plugins(InputManagerPlugin::<Action>::default())
            .add_systems(Startup, setup)
            .add_systems(Update, (grab_mouse, show_status, show_gesture));
    }
}

fn setup(mut commands: Commands, mut windows: Query<&mut Window>) -> Result<(), BevyError> {
    let mut window = windows.single_mut().unwrap();
    window.title = "None".to_string();

    commands.spawn((Camera2d, Camera { ..default() }));

    let input_map = InputMap::new()
        .with_binding(
            Action::MouseClickPosition,
            DualAxisChord::new(MouseButtonInput::new(MouseButton::Left), MousePosition::new()),
        )?
        .with_binding(
            Action::CtrlMousePosition,
            DualAxisChord::new(KeyboardInput::new(KeyCode::ControlLeft), MousePosition::new()),
        )?
        .with_binding(Action::TouchPosition, TouchPosition::new())?
        .with_binding(Action::GestureTriangle, GestureInput::new(GestureId(9)))?
        .with_binding(Action::GestureRectangle, GestureInput::new(GestureId(10)))?
        .with_binding(Action::GestureCircle, GestureInput::new(GestureId(11)))?
        .with_binding(Action::Grab, KeyboardInput::new(KeyCode::Space))?;

    // Template-Set can be saved and loaded to speed up the setup.
    let mut template_set = JackknifeTemplateSet::new(JackknifeConfig::inner_product());
    for (_, points, id) in GESTURES {
        template_set.add_template(*id, points);
    }

    template_set.train(1000, 16, 0.25, 4, 0.4, None);

    let root = commands
        .spawn((
            input_map,
            GestureSet(template_set),
            UnistrokeGesture::new(Action::MouseClickPosition, 10.0),
            Text::default(),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
                ..default()
            },
        ))
        .id();

    // Spawn some more recognizers for different input devices
    commands.spawn((
        AttachedToGestureSet(root),
        UnistrokeGesture::new(Action::TouchPosition, 10.0),
    ));
    commands.spawn((
        AttachedToGestureSet(root),
        UnistrokeGesture::new(Action::CtrlMousePosition, 10.0),
    ));

    Ok(())
}

fn grab_mouse(
    players: Query<&ActionState<Action>>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut cursor_options: Query<&mut CursorOptions, With<PrimaryWindow>>,
) -> Result<(), BevyError> {
    let action_state = players.single()?;
    let mut cursor_options = cursor_options.single_mut()?;
    let mut window = window.single_mut()?;

    if action_state.just_pressed(&Action::Grab) {
        match cursor_options.grab_mode {
            CursorGrabMode::None => {
                cursor_options.grab_mode = CursorGrabMode::Locked;
                window.title = "Locked".to_string();
            }
            CursorGrabMode::Locked => {
                cursor_options.grab_mode = CursorGrabMode::Confined;
                window.title = "Confined".to_string();
            }
            CursorGrabMode::Confined => {
                cursor_options.grab_mode = CursorGrabMode::None;
                window.title = "None".to_string();
            }
        };
    }

    Ok(())
}

#[derive(Default)]
struct GestureHistory {
    last_gesture_time: HashMap<Action, f32>,
}

fn show_gesture(
    gesture_q: Query<&GestureSet>,
    recognizer_q: Query<&UnistrokeGesture<Action>>,
    window: Query<&Window>,
    mut gizmos: Gizmos,
) -> Result<(), BevyError> {
    let window = window.single()?;
    let (width, height) = (window.width(), window.height());

    let gesture_set = gesture_q.single()?;

    for recognizer in recognizer_q.iter() {
        // raw input
        let points = recognizer.points();
        let dt = Vec3::new(-width / 2.0, height / 2.0, 0.0);
        gizmos.linestrip(
            points.iter().map(|p| Vec3::new(p.x, -p.y, 0.0) + dt),
            Color::srgb(1.0, 1.0, 1.0),
        );

        let mut resampled_points = Vec::new();
        let mut gesture_points = Vec::new();

        let map_point = move |p: &Vec2| -> Vec2 { Vec2::new(p.x - width / 2.0, -p.y + height / 3.0) * 0.8 };

        // resampled input
        if let Some(points) = recognizer.resampled_points() {
            resampled_points = points.iter().map(&map_point).collect::<Vec<_>>();

            let cs = 1.0 / points.len() as f32;
            gizmos.linestrip_gradient_2d(
                resampled_points
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (*p, Color::srgb((i as f32) * cs, 0.0, (i as f32) * cs))),
            );

            let random = Vec2::stochastic_variance(&resampled_points, 16, 0.25, 4, 0.4);
            let cs = 1.0 / points.len() as f32;
            gizmos.linestrip_gradient_2d(
                random
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (*p, Color::srgb((i as f32) * cs, 1.0, (i as f32) * cs))),
            );
        }

        // detected gesture
        if let Some((gesture_id, gesture_index, _)) = recognizer.classification() {
            for (index, template) in gesture_set
                .templates()
                .iter()
                .enumerate()
                .filter(|(_, t)| t.id() == gesture_id)
            {
                let points = template.resampled_points();

                if index == gesture_index {
                    gesture_points = points.iter().map(&map_point).collect::<Vec<_>>();
                }

                let cs = 1.0 / points.len() as f32;
                gizmos.linestrip_gradient_2d(points.iter().enumerate().map(|(i, p)| {
                    (
                        map_point(p),
                        if index == gesture_index {
                            Color::srgb(0.0, (i as f32) * cs, 0.0)
                        } else {
                            Color::srgb(0.0, (i as f32) * cs, (i as f32) * cs)
                        },
                    )
                }));
            }
        }

        if !gesture_points.is_empty() && !resampled_points.is_empty() {
            let cs = 1.0 / gesture_points.len() as f32;
            let matching = recognizer.internal().cost_matrix.matching();
            for (i, j) in matching {
                if i < resampled_points.len() && j < gesture_points.len() {
                    gizmos.line_2d(
                        resampled_points[i],
                        gesture_points[j],
                        Color::srgb((i as f32) * cs, 0.0, 0.0),
                    );
                }
            }
        }
    }

    Ok(())
}

fn show_status(
    mut players: Query<(&ActionState<Action>, &mut Text)>,
    time: Res<Time>,
    mut gesture_history: Local<GestureHistory>,
) {
    for (action_state, mut text) in players.iter_mut() {
        for action in [Action::GestureCircle, Action::GestureTriangle, Action::GestureRectangle] {
            if action_state.just_pressed(&action) {
                gesture_history.last_gesture_time.insert(action, time.elapsed_secs());
            } else {
                gesture_history.last_gesture_time.entry(action).or_insert(f32::INFINITY);
            }
        }
        let gesture_time = gesture_history
            .last_gesture_time
            .iter()
            .map(|(action, &t)| {
                format!(
                    "{:?}: {:.2}s",
                    action,
                    if t != f32::INFINITY {
                        (time.elapsed_secs() - t).max(0.0)
                    } else {
                        t
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        text.0 = [gesture_time].join("\n");
    }
}
