use bevy::{prelude::*, window::CursorGrabMode};
use shine_game::{
    application,
    input_manager::{
        ActionState, EdgeSize, GamepadButtonInput, GamepadStick, GamepadStickInput, InputManagerPlugin, InputMap,
        KeyboardInput, MouseButtonInput, MouseMotion, MousePosition, PinchCenter, PinchPan, PinchRotate, PinchZoom,
        ScreenPositionProcessor, TouchPosition,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    MouseLeft,
    MouseMiddle,
    MouseRight,
    MouseMotion,
    MousePosition,
    MouseNormalizedPosition,
    MouseEdgeScroll,

    TouchPosition,
    TouchNormalizedPosition,
    TouchEdgeScroll,

    PinchPan,
    PinchPanTotal,
    PinchZoom,
    PinchZoomTotal,
    PinchRotate,
    PinchRotateTotal,
    PinchCenter,

    GamePadLeftStick,
    GamePadRightStick,
    GamePadLeftTrigger,
    GamePadRightTrigger,

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
        .with_button(Action::MouseLeft, MouseButtonInput::new(MouseButton::Left))
        .with_button(Action::MouseMiddle, MouseButtonInput::new(MouseButton::Middle))
        .with_button(Action::MouseRight, MouseButtonInput::new(MouseButton::Right))
        .with_dual_axis(Action::MouseMotion, MouseMotion::new())
        .with_dual_axis(Action::MousePosition, MousePosition::new())
        .with_dual_axis(
            Action::MouseNormalizedPosition,
            MousePosition::new().normalize_to_screen(),
        )
        .with_dual_axis(
            Action::MouseEdgeScroll,
            MousePosition::new().edge_scroll(EdgeSize::Fixed(50.)),
        )
        .with_dual_axis(Action::TouchPosition, TouchPosition::new())
        .with_dual_axis(
            Action::TouchNormalizedPosition,
            TouchPosition::new().normalize_to_screen(),
        )
        .with_dual_axis(
            Action::TouchEdgeScroll,
            TouchPosition::new().edge_scroll(EdgeSize::Fixed(50.)),
        )
        .with_dual_axis(Action::PinchPan, PinchPan::delta())
        .with_dual_axis(Action::PinchPanTotal, PinchPan::total())
        .with_axis(Action::PinchZoom, PinchZoom::delta())
        .with_axis(Action::PinchZoomTotal, PinchZoom::total())
        .with_axis(Action::PinchRotate, PinchRotate::delta())
        .with_axis(Action::PinchRotateTotal, PinchRotate::total())
        .with_dual_axis(Action::PinchCenter, PinchCenter::new())
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
            log::info!("Player joined gamepad {gamepad_entity}");
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
                    Action::GamePadLeftTrigger,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::LeftTrigger),
                )
                .add_button(
                    Action::GamePadRightTrigger,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::RightTrigger),
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
            format!("Size: {width}x{height}")
        };

        let mouse_button = format!(
            "Mouse Button: {:?} {:?} {:?}",
            action_state.button_value(&Action::MouseLeft),
            action_state.button_value(&Action::MouseMiddle),
            action_state.button_value(&Action::MouseRight)
        );
        let mouse_motion = format!("Mouse Motion: {:?}", action_state.dual_axis_value(&Action::MouseMotion));
        let mouse_position = format!(
            "Mouse Position: {} ({})",
            action_state
                .try_dual_axis_value(&Action::MousePosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_dual_axis_value(&Action::MouseNormalizedPosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string())
        );
        let mouse_edge_scroll = match action_state.try_dual_axis_value(&Action::MouseEdgeScroll) {
            None => "Edge Scroll: None".to_string(),
            Some(value) => format!("Edge Scroll: {value:?}"),
        };

        let touch_position = format!(
            "Touch Position: {} ({})",
            action_state
                .try_dual_axis_value(&Action::TouchPosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_dual_axis_value(&Action::TouchNormalizedPosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string())
        );
        let touch_edge_scroll = match action_state.try_dual_axis_value(&Action::TouchEdgeScroll) {
            None => "Touch Edge Scroll: None".to_string(),
            Some(value) => format!("Touch Edge Scroll: {value:?}"),
        };

        let pinch_gesture = format!(
            "Pinch Pan: {} ({}), Zoom: {} ({}), Rotate: {} ({}), Center: {}",
            action_state
                .try_dual_axis_value(&Action::PinchPan)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_dual_axis_value(&Action::PinchPanTotal)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_axis_value(&Action::PinchZoom)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_axis_value(&Action::PinchZoomTotal)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_axis_value(&Action::PinchRotate)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_axis_value(&Action::PinchRotateTotal)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_dual_axis_value(&Action::PinchCenter)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string())
        );

        let gamepad_stick = format!(
            "Gamepad Sticks: {:?} {:?}",
            action_state
                .try_dual_axis_value(&Action::GamePadLeftStick)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_dual_axis_value(&Action::GamePadRightStick)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string())
        );
        let gamepad_trigger = format!(
            "GamePad Triggers: {:?} {:?}",
            action_state.button_value(&Action::GamePadLeftTrigger),
            action_state.button_value(&Action::GamePadRightTrigger)
        );

        text.0 = [
            size,
            mouse_button,
            mouse_motion,
            mouse_position,
            mouse_edge_scroll,
            touch_position,
            touch_edge_scroll,
            pinch_gesture,
            gamepad_stick,
            gamepad_trigger,
        ]
        .join("\n");
    }
}
