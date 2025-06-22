use bevy::prelude::*;
use shine_game::input_manager::{
    ActionLike, ActionState, CircleBoundsProcessor, InputManagerPlugin, InputMap, KeyboardInput, VirtualDpad,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    Movement,
    Fire,
}

impl ActionLike for Action {}

#[derive(Component)]
struct StatusText;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(InputManagerPlugin::<Action>::default())
        .add_systems(Startup, setup)
        .add_systems(Update, show_status)
        .run();
}

fn setup(mut commands: Commands, mut input_map: ResMut<InputMap<Action>>) {
    input_map.bind_dual_axis(
        Action::Movement,
        VirtualDpad::new(
            KeyboardInput::new(KeyCode::KeyW),
            KeyboardInput::new(KeyCode::KeyS),
            KeyboardInput::new(KeyCode::KeyA),
            KeyboardInput::new(KeyCode::KeyD),
        )
        .with_circle_bounds(1.0),
    );
    input_map.bind_button(Action::Fire, KeyboardInput::new(KeyCode::Space));
    input_map.bind_button(Action::Fire, KeyboardInput::new(KeyCode::Enter));

    commands.spawn((Camera2d, Camera { ..default() }));

    commands.spawn((
        Text::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}

/// System that shows the state of the `MoveForward` action on the screen.
fn show_status(action_state: Res<ActionState<Action>>, time: Res<Time>, mut text: Single<&mut Text>) {
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
