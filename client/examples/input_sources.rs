use bevy::{prelude::*, window::CursorGrabMode};
use shine_game::{
    application,
    input_manager::{
        ActionState, ButtonChord, DualAxisChord, EdgeSize, GamepadButtonInput, GamepadStick, GamepadStickInput,
        InputManagerPlugin, InputMap, KeyboardInput, MouseButtonInput, MouseMotionInput, MousePositionInput,
        ScreenPositionProcessor, TouchPositionInput,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    Motion,
    Position,
    EdgeScroll,
    NormalizedPosition,

    TouchPosition,
    TouchNormalizedPosition,
    TouchEdgeScroll,

    GamePadLeftStick,
    GamePadRightStick,
    GamePadRightTrigger,

    ButtonChardCtrlA,
    ButtonChardAB,

    DualAxisChordMouseLeft,
    DualAxisChordCtrlAGamepadLeftStick,

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
        .add_systems(Update, (grab_mouse, join_gamepad, show_status));
}

#[derive(Component)]
struct Player {
    gamepad: Option<Entity>,
}

fn setup(mut commands: Commands, mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut().unwrap();
    window.title = "None".to_string();

    commands.spawn((Camera2d, Camera { ..default() }));

    let input_map = InputMap::new()
        .with_dual_axis(Action::Motion, MouseMotionInput::new())
        .with_dual_axis(Action::Position, MousePositionInput::new())
        .with_dual_axis(
            Action::NormalizedPosition,
            MousePositionInput::new().normalize_to_screen(),
        )
        .with_dual_axis(
            Action::EdgeScroll,
            MousePositionInput::new().edge_scroll(EdgeSize::Fixed(50.)),
        )
        .with_dual_axis(Action::TouchPosition, TouchPositionInput::new())
        .with_dual_axis(
            Action::TouchNormalizedPosition,
            TouchPositionInput::new().normalize_to_screen(),
        )
        .with_dual_axis(
            Action::TouchEdgeScroll,
            TouchPositionInput::new().edge_scroll(EdgeSize::Fixed(50.)),
        )
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
            DualAxisChord::new(MouseButtonInput::new(MouseButton::Left), MouseMotionInput::new()),
        )
        .with_button(Action::Grab, KeyboardInput::new(KeyCode::Space));

    commands.spawn((
        Player { gamepad: None },
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

fn join_gamepad(
    gamepads_q: Query<(Entity, &Gamepad)>,
    mut player_q: Query<(&mut InputMap<Action>, &mut Player)>,
) -> Result<(), BevyError> {
    let (mut input, mut player) = player_q.single_mut()?;

    if player.gamepad.is_none() {
        if let Some((gamepad_entity, _)) = gamepads_q.iter().next() {
            log::info!("Player joined gamepad {}", gamepad_entity);
            player.gamepad = Some(gamepad_entity);
            input
                .add_dual_axis(
                    Action::GamePadLeftStick,
                    GamepadStickInput::new(gamepad_entity, GamepadStick::Left),
                )
                .add_dual_axis(
                    Action::GamePadRightStick,
                    GamepadStickInput::new(gamepad_entity, GamepadStick::Right),
                )
                .add_button(
                    Action::GamePadRightTrigger,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::RightTrigger),
                )
                .add_dual_axis(
                    Action::DualAxisChordCtrlAGamepadLeftStick,
                    DualAxisChord::new(
                        ButtonChord::new2(
                            KeyboardInput::new(KeyCode::ControlLeft),
                            KeyboardInput::new(KeyCode::KeyA),
                        ),
                        GamepadStickInput::new(gamepad_entity, GamepadStick::Left),
                    ),
                );
        }
    }

    Ok(())
}

fn show_status(mut players: Query<(&ActionState<Action>, &mut Text)>, window: Query<&Window>) {
    for (action_state, mut text) in players.iter_mut() {
        let size = {
            let window = window.single().unwrap();
            let (width, height) = (window.width(), window.height());
            format!("Size: {}x{}", width, height)
        };

        let motion = format!("Motion: {:?}", action_state.dual_axis_value(&Action::Motion));
        let position = match action_state.try_dual_axis_value(&Action::Position) {
            None => "Position: None".to_string(),
            Some(value) => format!("Position: {:?}", value),
        };
        let normalized_position = match action_state.try_dual_axis_value(&Action::NormalizedPosition) {
            None => "Normalized Position: None".to_string(),
            Some(value) => format!("Normalized Position: {:?}", value),
        };
        let edge_scroll = match action_state.try_dual_axis_value(&Action::EdgeScroll) {
            None => "Edge Scroll: None".to_string(),
            Some(value) => format!("Edge Scroll: {:?}", value),
        };

        let touch_position = match action_state.try_dual_axis_value(&Action::TouchPosition) {
            None => "Touch Position: None".to_string(),
            Some(value) => format!("Touch Position: {:?}", value),
        };
        let touch_normalized_position = match action_state.try_dual_axis_value(&Action::TouchNormalizedPosition) {
            None => "Touch Normalized Position: None".to_string(),
            Some(value) => format!("Touch Normalized Position: {:?}", value),
        };
        let touch_edge_scroll = match action_state.try_dual_axis_value(&Action::TouchEdgeScroll) {
            None => "Touch Edge Scroll: None".to_string(),
            Some(value) => format!("Touch Edge Scroll: {:?}", value),
        };

        let gamepad_left_stick = match action_state.try_dual_axis_value(&Action::GamePadLeftStick) {
            None => "GamePad Left Stick: None".to_string(),
            Some(value) => format!("GamePad Left Stick: {:?}", value),
        };
        let gamepad_right_stick = match action_state.try_dual_axis_value(&Action::GamePadRightStick) {
            None => "GamePad Right Stick: None".to_string(),
            Some(value) => format!("GamePad Right Stick: {:?}", value),
        };
        let gamepad_right_trigger = format!(
            "GamePad Right Trigger: {:?}",
            action_state.button_value(&Action::GamePadRightTrigger)
        );

        let button_chord_ab = format!(
            "Button Chord A+B: {:?}",
            action_state.button_value(&Action::ButtonChardAB)
        );
        let button_chord_ctrl_a = format!(
            "Button Chord Ctrl+A: {:?}",
            action_state.button_value(&Action::ButtonChardCtrlA)
        );

        let dual_axis_chord_mouse_left = match action_state.try_dual_axis_value(&Action::DualAxisChordMouseLeft) {
            None => "Mouse Left + Move: None".to_string(),
            Some(value) => format!("Mouse Left + Move: {:?}", value),
        };
        let dual_axis_chord_ctrl_a_gamepad_left =
            match action_state.try_dual_axis_value(&Action::DualAxisChordCtrlAGamepadLeftStick) {
                None => "Ctrl-A + GamePad Left Stick: None".to_string(),
                Some(value) => format!("Ctrl-A + GamePad Left Stick: {:?}", value),
            };

        text.0 = [
            size,
            motion,
            position,
            normalized_position,
            edge_scroll,
            touch_position,
            touch_normalized_position,
            touch_edge_scroll,
            gamepad_left_stick,
            gamepad_right_stick,
            gamepad_right_trigger,
            button_chord_ab,
            button_chord_ctrl_a,
            dual_axis_chord_mouse_left,
            dual_axis_chord_ctrl_a_gamepad_left,
        ]
        .join("\n");
    }
}
