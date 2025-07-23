use bevy::{prelude::*, window::CursorGrabMode};
use shine_game::{
    ai::unistroke_templates,
    application,
    input_manager::{
        ActionState, BoxedDualAxisLike, ButtonChord, ButtonCompose, ClassificationCompose, DualAxisChord,
        DualAxisCompose, InputManagerPlugin, InputMap, KeyboardInput, MouseButtonInput, MouseMotion, MousePosition,
        TouchPosition, UnistrokeGesture, UserInputExt, VirtualDPad,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    EitherABMouseLeft,
    MaxMouseVPad,

    ButtonChardCtrlA,
    ButtonChardAB,

    DualAxisChordMouseLeft,
    DualAxisChordCtrlAMousePosition,

    Gesture,

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

const GESTURES: &[(&str, &[Vec2])] = &[
    ("Line 0", unistroke_templates::LINE_0),
    ("Line 45", unistroke_templates::LINE_45),
    ("Line 90", unistroke_templates::LINE_90),
    ("Line 135", unistroke_templates::LINE_135),
    ("Line 180", unistroke_templates::LINE_180),
    ("Line 225", unistroke_templates::LINE_225),
    ("Line 270", unistroke_templates::LINE_270),
    ("Line 315", unistroke_templates::LINE_315),
    ("V", unistroke_templates::V),
    ("Triangle", unistroke_templates::TRIANGLE),
    ("Rectangle", unistroke_templates::RECTANGLE),
    ("Circle", unistroke_templates::CIRCLE),
    ("Zig Zag", unistroke_templates::ZIG_ZAG),
];

fn setup_game(app: &mut App) {
    app.add_plugins(InputManagerPlugin::<Action>::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (grab_mouse, show_status));
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
        .with_classification(Action::Gesture, {
            let mut gesture = UnistrokeGesture::new(
                // boxed to type erase and access the gesture with partial type information
                DualAxisChord::new(MouseButtonInput::new(MouseButton::Left), MousePosition::new()).boxed(),
                10.0,
            );
            for (_, template) in GESTURES {
                gesture = gesture.with_gesture(template);
            }

            let mut gesture2 = UnistrokeGesture::new(
                // boxed to type erase and access the gesture with partial type information
                TouchPosition::new().boxed(),
                10.0,
            );
            for (_, template) in GESTURES {
                gesture2 = gesture2.with_gesture(template);
            }

            gesture.min(gesture2)
        })
        .with_button(Action::Grab, KeyboardInput::new(KeyCode::Space));

    for action in [
        Action::EitherABMouseLeft,
        Action::MaxMouseVPad,
        Action::ButtonChardAB,
        Action::ButtonChardCtrlA,
        Action::DualAxisChordMouseLeft,
        Action::DualAxisChordCtrlAMousePosition,
    ] {
        input_map.user_input(&action).map(|input| {
            let mut result = String::new();
            input.dump_pipeline(&mut result).unwrap();
            log::info!("{:?}:\n{}", action, result);
        });
    }

    commands.spawn((
        input_map,
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
pub struct LastGesture {
    pub start_time: f32,
    pub gesture: Option<(usize, f32)>,
}

impl LastGesture {
    pub fn elapsed_time(&self, time: &Time) -> f32 {
        (time.elapsed().as_secs_f32() - self.start_time).max(0.0)
    }
}

fn show_status(
    mut players: Query<(&InputMap<Action>, &ActionState<Action>, &mut Text)>,
    mut gizmos: Gizmos,
    window: Query<&Window>,
    time: Res<Time>,
    mut last_gesture: Local<LastGesture>,
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

        if let Some((index, score)) = action_state.try_classification_value(&Action::Gesture) {
            // quick hack
            if score > 2.0 {
                last_gesture.gesture = None;
            } else {
                last_gesture.gesture = Some((index, score));
            }
            last_gesture.start_time = time.elapsed().as_secs_f32();
        }
        let gesture = format!(
            "Last Gesture: {:?} ({:.2}s)",
            last_gesture
                .gesture
                .as_ref()
                .map(|(index, score)| format!("{} ({:.2})", GESTURES[*index].0, score))
                .unwrap_or_else(|| "None".to_string()),
            last_gesture.elapsed_time(&time)
        );

        if let Some(gesture) = input_map
            .classification(&Action::Gesture)
            .and_then(|c| c.find_by_name_as::<UnistrokeGesture>(""))
        {
            // raw input
            let points = gesture.points();
            let dt = Vec3::new(-width / 2.0, height / 2.0, 0.0);
            gizmos.linestrip(
                points.iter().map(|p| Vec3::new(p.x, -p.y, 0.0) + dt),
                Color::srgb(1.0, 1.0, 1.0),
            );

            // resampled input
            if let Some(points) = gesture.resampled_points() {
                let cs = 1.0 / points.len() as f32;
                gizmos.linestrip_gradient(points.iter().enumerate().map(|(i, p)| {
                    (
                        Vec3::new(p.x, -p.y, 0.0) * 0.5,
                        Color::srgb((i as f32) * cs, 0.0, (i as f32) * cs),
                    )
                }));
            }

            // detected gesture
            if let Some((index, _)) = last_gesture.gesture {
                let points = GESTURES[index].1;

                let cs = 1.0 / points.len() as f32;
                gizmos.linestrip_gradient(points.iter().enumerate().map(|(i, p)| {
                    (
                        Vec3::new(p.x, -p.y, 0.0) * 0.5,
                        Color::srgb(0.0, (i as f32) * cs, (i as f32) * cs),
                    )
                }));
            }
        }

        text.0 = [size, button_or, button_chord, dual_axis_chord, gesture].join("\n");
    }
}
