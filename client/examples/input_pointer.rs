use bevy::{prelude::*, window::CursorGrabMode};
use shine_game::{
    application,
    input_manager::{
        ActionState, EdgeSize, InputManagerPlugin, InputMap, KeyboardInput, MouseMotionInput, MousePositionInput,
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
        .add_systems(Update, (show_status, grab_mouse));
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

    let grab_value = action_state.button(&Action::Grab);
    if grab_value.just_pressed() {
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
        let size_str = {
            let window = window.single().unwrap();
            let (width, height) = (window.width(), window.height());
            format!("Size: {}x{}", width, height)
        };

        let motion_str = {
            let value = action_state.dual_axis(&Action::Motion);
            format!("Motion: {:?}", value.value)
        };

        let position_str = {
            let value = action_state.dual_axis(&Action::Position);
            format!("Position: {:?}", value.value)
        };

        let normalized_position_str = {
            let value = action_state.dual_axis(&Action::NormalizedPosition);
            format!("Normalized Position: {:?}", value.value)
        };

        let edge_scroll_str = {
            let value = action_state.dual_axis(&Action::EdgeScroll);
            format!("Edge Scroll: {:?}", value.value)
        };

        let touch_position_str = {
            let value = action_state.dual_axis(&Action::TouchPosition);
            if value.value == Vec2::MAX {
                "Touch Position: None".to_string()
            } else {
                format!("Touch - Position: {:?}", value.value)
            }
        };

        let touch_normalized_position_str = {
            let value = action_state.dual_axis(&Action::TouchNormalizedPosition);
            if value.value == Vec2::MAX {
                "Normalized Touch Position: None".to_string()
            } else {
                format!("Touch - Normalized Position: {:?}", value.value)
            }
        };

        let touch_edge_scroll_str = {
            let value = action_state.dual_axis(&Action::TouchEdgeScroll);
            format!("Touch - Edge Scroll: {:?}", value.value)
        };

        text.0 = [
            size_str,
            motion_str,
            position_str,
            normalized_position_str,
            edge_scroll_str,
            touch_position_str,
            touch_normalized_position_str,
            touch_edge_scroll_str,
        ]
        .join("\n");
    }
}
