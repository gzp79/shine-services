use bevy::{prelude::*, window::CursorGrabMode};
use core::f32;
use shine_game::{
    application,
    input_manager::{
        ActionState, DualAxisChord, GestureSet, InputManagerPlugin, InputMap, KeyboardInput, MouseButtonInput,
        MousePosition, UnistrokeGesture,
    },
    math::{unistroke_templates, GestureId, JackknifeConfig, JackknifeTemplateSet},
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    MousePosition,
    GestureCircle,
    GestureTriangle,
    GestureRectangle,

    Grab,
}

#[derive(Component)]
struct StatusText;

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

fn setup_game(app: &mut App) {
    app.add_plugins(InputManagerPlugin::<Action>::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (grab_mouse, show_status, show_gesture));
}

fn setup(mut commands: Commands, mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut().unwrap();
    window.title = "None".to_string();

    commands.spawn((Camera2d, Camera { ..default() }));

    let input_map = InputMap::new()
        .with_dual_axis(
            Action::MousePosition,
            DualAxisChord::new(MouseButtonInput::new(MouseButton::Left), MousePosition::new()),
        )
        .with_button(Action::Grab, KeyboardInput::new(KeyCode::Space));

    //let mut template_set = JackknifeTemplateSet::new(JackknifeConfig::inner_product());
    let mut template_set = JackknifeTemplateSet::new(JackknifeConfig::euclidean_distance());
    for (_, points, id) in GESTURES {
        template_set.add_template(*id, points);
    }

    commands.spawn((
        input_map,
        GestureSet { template_set },
        UnistrokeGesture::new(Action::MousePosition, 10.0)
            .with_button_target(GestureId(9), Action::GestureTriangle)
            .with_button_target(GestureId(10), Action::GestureRectangle)
            .with_button_target(GestureId(11), Action::GestureCircle),
        Text::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}

fn grab_mouse(players: Query<&ActionState<Action>, Without<Window>>, mut window: Query<&mut Window>) {
    let action_state = players.single().unwrap();
    let mut window = window.single_mut().unwrap();

    if action_state.just_pressed(&Action::Grab) {
        match window.cursor_options.grab_mode {
            CursorGrabMode::None => {
                window.cursor_options.grab_mode = CursorGrabMode::Locked;
                window.title = "Locked".to_string();
            }
            CursorGrabMode::Locked => {
                window.cursor_options.grab_mode = CursorGrabMode::Confined;
                window.title = "Confined".to_string();
            }
            CursorGrabMode::Confined => {
                window.cursor_options.grab_mode = CursorGrabMode::None;
                window.title = "None".to_string();
            }
        };
    }
}

#[derive(Default)]
struct GestureHistory {
    last_gesture_time: HashMap<Action, f32>,
}

fn show_gesture(
    gesture_q: Query<(&GestureSet, &UnistrokeGesture<Action>)>,
    window: Query<&Window>,
    mut gizmos: Gizmos,
) -> Result<(), BevyError> {
    let window = window.single()?;
    let (width, height) = (window.width(), window.height());

    let (gesture_set, recognizer) = gesture_q.single()?;

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
    }

    // detected gesture
    if let Some((gesture_id, gesture_index, _)) = recognizer.classification() {
        for (index, template) in gesture_set
            .template_set
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
