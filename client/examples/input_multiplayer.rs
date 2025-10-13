use bevy::{
    app::{App, Startup, Update},
    camera::{Camera, Camera2d},
    ecs::{
        component::Component,
        entity::Entity,
        error::BevyError,
        name::Name,
        query::{With, Without},
        system::{Commands, Query, Res},
    },
    input::{
        gamepad::{Gamepad, GamepadButton},
        keyboard::KeyCode,
        mouse::MouseButton,
    },
    tasks::BoxedFuture,
    time::Time,
    ui::{widget::Text, Node, PositionType, Val},
    utils::default,
};
use shine_game::{
    app::{init_application, platform, GameSetup, PlatformInit},
    input_manager::{
        ActionState, ButtonValue, GamepadButtonInput, GamepadStick, GamepadStickInput, InputManagerPlugin, InputMap,
        KeyboardInput, MouseButtonInput, RadialInputProcess, VirtualDPad,
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
            .add_systems(Update, (join_gamepad, show_status));
    }
}
fn setup(mut commands: Commands) -> Result<(), BevyError> {
    commands.spawn((Camera2d, Camera { ..default() }));

    // note due to keyboard ghosting, some action combinations may not work as expected
    // for example, pressing W + S + I may not register all the keys at once.
    // Use gaming keyboards :)

    let input_map_a = InputMap::new()
        .with_binding(Action::Movement, VirtualDPad::wasd())?
        .with_binding(Action::Fire, KeyboardInput::new(KeyCode::KeyZ))?
        .with_binding(Action::Fire, MouseButtonInput::new(MouseButton::Left))?;

    commands.spawn((
        Name::new("Player A"),
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
        .with_binding(Action::Movement, VirtualDPad::ijkl())?
        .with_binding(Action::Fire, KeyboardInput::new(KeyCode::KeyN))?;

    commands.spawn((
        Name::new("Player B"),
        PlayerB { gamepad: None },
        input_map_b,
        Text::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(92.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

    Ok(())
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
                .add_binding(Action::Movement, VirtualDPad::gamepad_dpad(gamepad_entity))?
                .add_binding(
                    Action::Movement,
                    GamepadStickInput::new(gamepad_entity, GamepadStick::Right)
                        .with_bounds(1.0)
                        .with_dead_zone(0.2),
                )?
                .add_binding(
                    Action::Fire,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::RightTrigger),
                )?;
        } else if player_b.gamepad.is_none() && player_a.gamepad != Some(gamepad_entity) {
            log::info!("Player B joined gamepad {gamepad_entity}");
            player_b.gamepad = Some(gamepad_entity);
            input_b
                .add_binding(Action::Movement, VirtualDPad::gamepad_dpad(gamepad_entity))?
                .add_binding(
                    Action::Movement,
                    GamepadStickInput::new(gamepad_entity, GamepadStick::Right)
                        .with_bounds(1.0)
                        .with_dead_zone(0.2),
                )?
                .add_binding(
                    Action::Fire,
                    GamepadButtonInput::new(gamepad_entity, GamepadButton::RightTrigger),
                )?;
        }
    }
    Ok(())
}

fn show_status(mut players: Query<(&ActionState<Action>, &Name, &mut Text)>, time: Res<Time>) {
    for (action_state, name, mut text) in players.iter_mut() {
        let mut logs = Vec::new();

        logs.push(name.to_string());
        logs.push(format!(
            "  Movement: {:?}",
            action_state.dual_axis_value(&Action::Movement)
        ));

        let fire_value = action_state.get_as::<ButtonValue>(&Action::Fire).unwrap();
        logs.push(format!(
            "  Fire: {:?} ({:.2}s)",
            fire_value.status,
            fire_value.elapsed_time(&time),
        ));

        text.0 = logs.join("\n");
    }
}
