use bevy::prelude::*;
use shine_game::input_manager::{
    ActionLike, ActionState, CircleBoundsProcessor, GamepadButtonInput, GamepadStickInput, InputManagerPlugin,
    InputMap, KeyboardInput, MouseButtonInput, VirtualDpad,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    Movement,
    Fire,
}

impl ActionLike for Action {}

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

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(InputManagerPlugin::<Action>::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (join_gamepad, show_status))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Camera2d, Camera { ..default() }));

    let input_map_a = InputMap::new()
        .with_dual_axis(
            Action::Movement,
            VirtualDpad::new(
                KeyboardInput::new(KeyCode::KeyW),
                KeyboardInput::new(KeyCode::KeyS),
                KeyboardInput::new(KeyCode::KeyA),
                KeyboardInput::new(KeyCode::KeyD),
            )
            .with_circle_bounds(1.0),
        )
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
        .with_dual_axis(
            Action::Movement,
            VirtualDpad::new(
                KeyboardInput::new(KeyCode::KeyI),
                KeyboardInput::new(KeyCode::KeyK),
                KeyboardInput::new(KeyCode::KeyJ),
                KeyboardInput::new(KeyCode::KeyL),
            )
            .with_circle_bounds(1.0),
        )
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
) {
    let (mut input_a, mut player_a) = player_a.single_mut().unwrap();
    let (mut input_b, mut player_b) = player_b.single_mut().unwrap();

    for (gamepad_entity, _) in gamepads.iter() {
        if player_a.gamepad.is_none() && player_b.gamepad != Some(gamepad_entity) {
            log::info!("Player A joined gamepad {}", gamepad_entity);
            player_a.gamepad = Some(gamepad_entity);
            input_a
                .add_dual_axis(
                    Action::Movement,
                    VirtualDpad::new(
                        GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadUp),
                        GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadDown),
                        GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadLeft),
                        GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadRight),
                    )
                    .with_circle_bounds(1.0),
                )
                .add_dual_axis(
                    Action::Movement,
                    GamepadStickInput::new(gamepad_entity, false)
                        .with_circle_bounds(1.0)
                        .with_circle_dead_zone(0.2),
                )
                .add_button(
                    Action::Fire,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::RightTrigger),
                );
        } else if player_b.gamepad.is_none() && player_a.gamepad != Some(gamepad_entity) {
            log::info!("Player B joined gamepad {}", gamepad_entity);
            player_b.gamepad = Some(gamepad_entity);
            input_b
                .add_dual_axis(
                    Action::Movement,
                    VirtualDpad::new(
                        GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadUp),
                        GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadDown),
                        GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadLeft),
                        GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadRight),
                    )
                    .with_circle_bounds(1.0),
                )
                .add_dual_axis(
                    Action::Movement,
                    GamepadStickInput::new(gamepad_entity, false)
                        .with_circle_bounds(1.0)
                        .with_circle_dead_zone(0.2),
                )
                .add_button(
                    Action::Fire,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::RightTrigger),
                );
        }
    }
}

fn show_status(mut players: Query<(&ActionState<Action>, &mut Text)>, time: Res<Time>) {
    for (action_state, mut text) in players.iter_mut() {
        let move_kind = action_state.kind(&Action::Movement);
        let move_value = action_state.dual_axis(&Action::Movement);

        let fire_kind = action_state.kind(&Action::Fire);
        let fire_value = action_state.button(&Action::Fire);

        text.0 = format!(
            "Movement: {:?} {:?}\nFire: {:?} {:?} ({:.2}s)",
            move_kind,
            move_value.value,
            fire_kind,
            fire_value.status,
            fire_value.elapsed_time(&*time)
        );
    }
}
