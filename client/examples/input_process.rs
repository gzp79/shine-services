use bevy::{prelude::*, window::CursorGrabMode};
use shine_game::{
    application,
    input_manager::{
        ActionState, ButtonChord, DualAxisChord, InputManagerPlugin, InputMap, KeyboardInput, MouseButtonInput,
        MouseMotion, MousePosition,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    ButtonChardCtrlA,
    ButtonChardAB,

    DualAxisChordMouseLeft,
    DualAxisChordCtrlAMousePosition,

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
        .with_button(Action::Grab, KeyboardInput::new(KeyCode::Space));

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

fn show_status(mut players: Query<(&ActionState<Action>, &mut Text)>, window: Query<&Window>) {
    for (action_state, mut text) in players.iter_mut() {
        let size = {
            let window = window.single().unwrap();
            let (width, height) = (window.width(), window.height());
            format!("Size: {width}x{height}")
        };

        let button_chord = format!(
            "Button Chord - A+B: {:?}, Ctrl+A: {:?}",
            action_state.button_value(&Action::ButtonChardAB),
            action_state.button_value(&Action::ButtonChardCtrlA)
        );

        let dual_axis_chord = format!(
            "DualAxis Chord - Mouse Left + Motion: {}, Ctrl-A + Mouse Position: {}",
            action_state
                .try_dual_axis_value(&Action::DualAxisChordMouseLeft)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_dual_axis_value(&Action::DualAxisChordCtrlAMousePosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string())
        );

        text.0 = [size, button_chord, dual_axis_chord].join("\n");
    }
}
