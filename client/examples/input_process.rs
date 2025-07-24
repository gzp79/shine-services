use bevy::{prelude::*, window::CursorGrabMode};
use core::f32;
use shine_game::{
    ai::{unistroke_templates, GestureId, JackknifeConfig, JackknifeTemplateSet},
    application,
    input_manager::{
        ActionState, ButtonChord, ButtonCompose, DualAxisChord, DualAxisCompose, GestureSet, InputManagerPlugin,
        InputMap, KeyboardInput, MouseButtonInput, MouseMotion, MousePosition, UnistrokeGesture, UserInputExt,
        VirtualDPad,
    },
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    EitherABMouseLeft,
    MaxMouseVPad,

    ButtonChardCtrlA,
    ButtonChardAB,

    DualAxisChordMouseLeft,
    DualAxisChordCtrlAMousePosition,

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
        .with_button(
            Action::EitherABMouseLeft,
            (KeyboardInput::new(KeyCode::KeyA).with_name("A"))
                .or(KeyboardInput::new(KeyCode::KeyB).with_name("B"))
                .or(MouseButtonInput::new(MouseButton::Left).with_name("Mouse left")),
        )
        .with_dual_axis(Action::MaxMouseVPad, MouseMotion::new().max(VirtualDPad::wasd()))
        .with_button(
            Action::ButtonChardAB,
            ButtonChord::new2(KeyboardInput::new(KeyCode::KeyA), KeyboardInput::new(KeyCode::KeyB)),
        )
        .with_button(
            Action::ButtonChardCtrlA,
            ButtonChord::new2(
                KeyboardInput::new(KeyCode::ControlLeft),
                KeyboardInput::new(KeyCode::KeyA),
            ),
        )
        .with_dual_axis(
            Action::DualAxisChordMouseLeft,
            DualAxisChord::new(MouseButtonInput::new(MouseButton::Left), MouseMotion::new()),
        )
        .with_dual_axis(
            Action::DualAxisChordCtrlAMousePosition,
            DualAxisChord::new(
                ButtonChord::new2(
                    KeyboardInput::new(KeyCode::ControlLeft),
                    KeyboardInput::new(KeyCode::KeyA),
                ),
                MousePosition::new(),
            ),
        )
        .with_dual_axis(
            Action::MousePosition,
            DualAxisChord::new(MouseButtonInput::new(MouseButton::Left), MousePosition::new()),
        )
        .with_button(Action::Grab, KeyboardInput::new(KeyCode::Space));

    let mut template_set = JackknifeTemplateSet::new(JackknifeConfig::inner_product());
    for (_, points, id) in GESTURES {
        template_set.add_template(*id, points);
    }

    for action in [
        Action::EitherABMouseLeft,
        Action::MaxMouseVPad,
        Action::ButtonChardAB,
        Action::ButtonChardCtrlA,
        Action::DualAxisChordMouseLeft,
        Action::DualAxisChordCtrlAMousePosition,
    ] {
        if let Some(input) = input_map.user_input(&action) {
            let mut result = String::new();
            input.dump_pipeline(&mut result).unwrap();
            log::info!("{action:?}:\n{result}");
        };
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

    // resampled input
    if let Some(points) = recognizer.resampled_points() {
        let cs = 1.0 / points.len() as f32;
        gizmos.linestrip_gradient(points.iter().enumerate().map(|(i, p)| {
            (
                Vec3::new(p.x, -p.y, 0.0) * 0.5,
                Color::srgb((i as f32) * cs, 0.0, (i as f32) * cs),
            )
        }));
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
            gizmos.linestrip_gradient(points.iter().enumerate().map(|(i, p)| {
                (
                    Vec3::new(p.x, -p.y, 0.0) * 0.5,
                    if index == gesture_index {
                        Color::srgb(0.0, (i as f32) * 0.1, 0.0)
                    } else {
                        Color::srgb(0.0, (i as f32) * 0.1, (i as f32) * 0.1)
                    },
                )
            }));
        }
    }

    Ok(())
}

fn show_status(
    mut players: Query<(&InputMap<Action>, &ActionState<Action>, &mut Text)>,
    window: Query<&Window>,
    time: Res<Time>,
    mut gesture_history: Local<GestureHistory>,
) {
    for (input_map, action_state, mut text) in players.iter_mut() {
        let window = window.single().unwrap();
        let (width, height) = (window.width(), window.height());

        let size = { format!("Size: {width}x{height}") };

        let button_or = {
            let a = input_map
                .button(&Action::EitherABMouseLeft)
                .and_then(|b| b.find_by_name_as::<KeyboardInput>("A"))
                .map(|b| b.is_pressed());
            let b = input_map
                .button(&Action::EitherABMouseLeft)
                .and_then(|b| b.find_by_name_as::<KeyboardInput>("B"))
                .map(|b| b.is_pressed());
            let left = input_map
                .button(&Action::EitherABMouseLeft)
                .and_then(|b| b.find_by_name_as::<MouseButtonInput>("Mouse left"))
                .map(|b| b.is_pressed());
            format!(
                "Or - A, B, Mouse left: {:?} ({:?}, {:?}, {:?})",
                action_state.button_value(&Action::EitherABMouseLeft),
                a,
                b,
                left
            )
        };

        let button_chord = format!(
            "Button Chord - A+B: {:?}\n   Ctrl+A: {:?}",
            action_state.button_value(&Action::ButtonChardAB),
            action_state.button_value(&Action::ButtonChardCtrlA)
        );

        let dual_axis_chord = format!(
            "DualAxis Chord - Mouse Left + Motion: {}\n   Ctrl-A + Mouse Position: {}",
            action_state
                .try_dual_axis_value(&Action::DualAxisChordMouseLeft)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_dual_axis_value(&Action::DualAxisChordCtrlAMousePosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string())
        );

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

        text.0 = [size, button_or, button_chord, dual_axis_chord, gesture_time].join("\n");
    }
}
