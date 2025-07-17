use bevy::prelude::*;
use shine_game::{
    application,
    input_manager::{
        ActionState, DualAxisRadialProcessor, GamepadButtonInput, GamepadStick, GamepadStickInput, InputManagerPlugin,
        InputMap, KeyboardInput, MouseButtonInput, VirtualDPad,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    Movement,
    Fire,
}

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct PlayerA {
    gamepad: Option<Entity>,
}

#[derive(Component)]
struct PlayerB {
    gamepad: Option<Entity>,
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

fn setup_game(app: &mut App) {
    app.add_plugins(InputManagerPlugin::<Action>::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (join_gamepad, show_status));
}

fn setup(mut commands: Commands) {
    commands.spawn((Camera2d, Camera { ..default() }));

    let input_map_a = InputMap::new()
        .with_dual_axis(Action::Movement, VirtualDPad::wasd())
        .with_button(Action::Fire, KeyboardInput::new(KeyCode::KeyZ))
        .with_button(Action::Fire, MouseButtonInput::new(MouseButton::Left));

    commands.spawn((
        PlayerA { gamepad: None },
        input_map_a,
        Text::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

    let input_map_b = InputMap::new()
        .with_dual_axis(Action::Movement, VirtualDPad::ijkl())
        .with_button(Action::Fire, KeyboardInput::new(KeyCode::KeyN));

    commands.spawn((
        PlayerB { gamepad: None },
        input_map_b,
        Text::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            right: Val::Px(12.0),
            ..default()
        },
    ));
}

fn join_gamepad(
    gamepads: Query<(Entity, &Gamepad)>,
    mut player_a: Query<(&mut InputMap<Action>, &mut PlayerA), (With<PlayerA>, Without<PlayerB>)>,
    mut player_b: Query<(&mut InputMap<Action>, &mut PlayerB), (With<PlayerB>, Without<PlayerA>)>,
) -> Result<(), BevyError> {
    let (mut input_a, mut player_a) = player_a.single_mut()?;
    let (mut input_b, mut player_b) = player_b.single_mut()?;

    for (gamepad_entity, _) in gamepads.iter() {
        if player_a.gamepad.is_none() && player_b.gamepad != Some(gamepad_entity) {
            log::info!("Player A joined gamepad {gamepad_entity}");
            player_a.gamepad = Some(gamepad_entity);
            input_a
                .add_dual_axis(Action::Movement, VirtualDPad::gamepad_dpad(gamepad_entity))
                .add_dual_axis(
                    Action::Movement,
                    GamepadStickInput::new(gamepad_entity, GamepadStick::Right)
                        .with_bounds(1.0)
                        .with_dead_zone(0.2),
                )
                .add_button(
                    Action::Fire,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::RightTrigger),
                );
        } else if player_b.gamepad.is_none() && player_a.gamepad != Some(gamepad_entity) {
            log::info!("Player B joined gamepad {gamepad_entity}");
            player_b.gamepad = Some(gamepad_entity);
            input_b
                .add_dual_axis(Action::Movement, VirtualDPad::gamepad_dpad(gamepad_entity))
                .add_dual_axis(
                    Action::Movement,
                    GamepadStickInput::new(gamepad_entity, GamepadStick::Right)
                        .with_bounds(1.0)
                        .with_dead_zone(0.2),
                )
                .add_button(
                    Action::Fire,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::RightTrigger),
                );
        }
    }
    Ok(())
}

fn show_status(mut players: Query<(&ActionState<Action>, &mut Text)>, time: Res<Time>) {
    for (action_state, mut text) in players.iter_mut() {
        let move_kind = action_state.kind(&Action::Movement);
        let move_value = action_state.dual_axis_value(&Action::Movement);

        let fire_kind = action_state.kind(&Action::Fire);
        let fire_value = action_state.as_button(&Action::Fire).cloned().unwrap_or_default();

        text.0 = format!(
            "Movement: {:?} {:?}\nFire: {:?} {:?} ({:.2}s)",
            move_kind,
            move_value,
            fire_kind,
            fire_value.status,
            fire_value.elapsed_time(&time),
        );
    }
}
