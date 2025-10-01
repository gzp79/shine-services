use bevy::{
    app::{App, Startup, Update},
    camera::{Camera, Camera2d},
    ecs::{
        component::Component,
        entity::Entity,
        error::BevyError,
        query::With,
        system::{Commands, Query},
    },
    input::{
        gamepad::{Gamepad, GamepadButton},
        keyboard::KeyCode,
        mouse::MouseButton,
    },
    ui::{widget::Text, Node, PositionType, Val},
    utils::default,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window},
};
use shine_game::{
    app::init_application,
    input_manager::{
        ActionState, EdgeSize, GamepadButtonInput, GamepadStick, GamepadStickInput, InputManagerPlugin, InputMap,
        KeyboardInput, MouseButtonInput, MouseMotion, MousePosition, ScreenPositionProcess, TouchPosition,
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
    use shine_game::app::{create_application, platform::Config};

    init_application(setup_game);
    let mut app = create_application(Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    init_application(setup_game);
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

fn setup(mut commands: Commands, mut windows: Query<&mut Window>) -> Result<(), BevyError> {
    let mut window = windows.single_mut().unwrap();
    window.title = "None".to_string();

    commands.spawn((Camera2d, Camera { ..default() }));

    let input_map = InputMap::new()
        .with_binding(Action::MouseLeft, MouseButtonInput::new(MouseButton::Left))?
        .with_binding(Action::MouseMiddle, MouseButtonInput::new(MouseButton::Middle))?
        .with_binding(Action::MouseRight, MouseButtonInput::new(MouseButton::Right))?
        .with_binding(Action::MouseMotion, MouseMotion::new())?
        .with_binding(Action::MousePosition, MousePosition::new())?
        .with_binding(
            Action::MouseNormalizedPosition,
            MousePosition::new().normalize_to_screen(),
        )?
        .with_binding(
            Action::MouseEdgeScroll,
            MousePosition::new().edge_scroll(EdgeSize::Fixed(50.)),
        )?
        .with_binding(Action::TouchPosition, TouchPosition::new())?
        .with_binding(
            Action::TouchNormalizedPosition,
            TouchPosition::new().normalize_to_screen(),
        )?
        .with_binding(
            Action::TouchEdgeScroll,
            TouchPosition::new().edge_scroll(EdgeSize::Fixed(50.)),
        )?
        .with_binding(Action::Grab, KeyboardInput::new(KeyCode::Space))?;

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
                .add_binding(
                    Action::GamePadLeftStick,
                    GamepadStickInput::new(gamepad_entity, GamepadStick::Left),
                )?
                .add_binding(
                    Action::GamePadRightStick,
                    GamepadStickInput::new(gamepad_entity, GamepadStick::Right),
                )?
                .add_binding(
                    Action::GamePadLeftTrigger,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::LeftTrigger),
                )?
                .add_binding(
                    Action::GamePadRightTrigger,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::RightTrigger),
                )?;
        }
    }

    Ok(())
}

fn show_status(mut players: Query<(&ActionState<Action>, &mut Text)>, window: Query<&Window>) {
    for (action_state, mut text) in players.iter_mut() {
        let mut logs = Vec::new();

        let window = window.single().unwrap();
        let (width, height) = (window.width(), window.height());
        logs.push(format!("Size: {width}x{height}"));

        logs.push(format!(
            "Mouse Button: {:?} {:?} {:?}",
            action_state.button_value(&Action::MouseLeft),
            action_state.button_value(&Action::MouseMiddle),
            action_state.button_value(&Action::MouseRight)
        ));
        logs.push(format!(
            "Mouse Motion: {:?}",
            action_state.dual_axis_value(&Action::MouseMotion)
        ));
        logs.push(format!(
            "Mouse Position: {} ({})",
            action_state
                .try_dual_axis_value(&Action::MousePosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_dual_axis_value(&Action::MouseNormalizedPosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string())
        ));
        match action_state.try_dual_axis_value(&Action::MouseEdgeScroll) {
            None => logs.push("Edge Scroll: None".to_string()),
            Some(value) => logs.push(format!("Edge Scroll: {value:?}")),
        };

        logs.push(format!(
            "Touch Position: {} ({})",
            action_state
                .try_dual_axis_value(&Action::TouchPosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_dual_axis_value(&Action::TouchNormalizedPosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string())
        ));
        match action_state.try_dual_axis_value(&Action::TouchEdgeScroll) {
            None => logs.push("Touch Edge Scroll: None".to_string()),
            Some(value) => logs.push(format!("Touch Edge Scroll: {value:?}")),
        };

        logs.push(format!(
            "Gamepad Sticks: {:?} {:?}",
            action_state
                .try_dual_axis_value(&Action::GamePadLeftStick)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
            action_state
                .try_dual_axis_value(&Action::GamePadRightStick)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string())
        ));
        logs.push(format!(
            "GamePad Triggers: {:?} {:?}",
            action_state.button_value(&Action::GamePadLeftTrigger),
            action_state.button_value(&Action::GamePadRightTrigger)
        ));

        text.0 = logs.join("\n");
    }
}
