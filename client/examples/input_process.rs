use bevy::{
    app::{App, Startup, Update},
    camera::{Camera, Camera2d},
    ecs::{
        component::Component,
        error::BevyError,
        query::With,
        system::{Commands, Query},
    },
    input::{keyboard::KeyCode, mouse::MouseButton},
    ui::{widget::Text, Node, PositionType, Val},
    utils::default,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window},
};
use shine_game::{
    app::init_application,
    input_manager::{
        ActionState, ButtonChord, DualAxisChord, InputManagerPlugin, InputMap, InputPipelineExt, KeyboardInput,
        MouseButtonInput, MouseMotion, MousePosition, VirtualDPad, VirtualPad,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    MultiBindABLeftMouse,

    VirtualDPad,
    VirtualPad,

    ButtonChordAB,
    ButtonChordCtrlA,
    DualAxisChordMouseLeft,
    DualAxisChordCtrlAMousePosition,

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
        .add_systems(Update, (grab_mouse, show_status));
}

fn setup(mut commands: Commands, mut windows: Query<&mut Window>) -> Result<(), BevyError> {
    let mut window = windows.single_mut().unwrap();
    window.title = "None".to_string();

    commands.spawn((Camera2d, Camera { ..default() }));

    let input_map = InputMap::new()
        .with_binding(Action::MultiBindABLeftMouse, KeyboardInput::new(KeyCode::KeyA))?
        .with_binding(Action::MultiBindABLeftMouse, KeyboardInput::new(KeyCode::KeyB))?
        .with_binding(Action::MultiBindABLeftMouse, MouseButtonInput::new(MouseButton::Left))?
        .with_binding(Action::VirtualPad, VirtualPad::from_keys(KeyCode::KeyQ, KeyCode::KeyE))?
        .with_binding(Action::VirtualDPad, VirtualDPad::wasd())?
        .with_binding(
            Action::ButtonChordAB,
            ButtonChord::new(KeyboardInput::new(KeyCode::KeyA), KeyboardInput::new(KeyCode::KeyB)),
        )?
        .with_binding(
            Action::ButtonChordCtrlA,
            ButtonChord::new(
                KeyboardInput::new(KeyCode::ControlLeft),
                KeyboardInput::new(KeyCode::KeyA),
            ),
        )?
        .with_binding(
            Action::DualAxisChordMouseLeft,
            DualAxisChord::new(MouseButtonInput::new(MouseButton::Left), MouseMotion::new()),
        )?
        .with_binding(
            Action::DualAxisChordCtrlAMousePosition,
            DualAxisChord::new(
                ButtonChord::new(
                    KeyboardInput::new(KeyCode::ControlLeft),
                    KeyboardInput::new(KeyCode::KeyA),
                ),
                MousePosition::new(),
            ),
        )?
        .with_binding(Action::Grab, KeyboardInput::new(KeyCode::Space))?;

    // dump some of the more complex pipelines
    for action in [
        Action::VirtualDPad,
        Action::ButtonChordCtrlA,
        Action::DualAxisChordCtrlAMousePosition,
        Action::MultiBindABLeftMouse,
    ] {
        if let Some(input) = input_map.get_pipeline(&action) {
            let mut result = String::new();
            input.dump(&mut result).unwrap();
            log::info!("{action:?}:\n{result}");
        };
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

fn show_status(mut players: Query<(&ActionState<Action>, &mut Text)>) {
    for (action_state, mut text) in players.iter_mut() {
        let mut logs = Vec::new();

        logs.push(format!(
            "Multi-bind to A, B, Mouse left: {:?}",
            action_state.button_value(&Action::MultiBindABLeftMouse)
        ));

        logs.push(format!(
            "VirtualPad QE: {:?}",
            action_state.axis_value(&Action::VirtualPad)
        ));
        logs.push(format!(
            "VirtualDPad WASD: {:?}",
            action_state.dual_axis_value(&Action::VirtualDPad)
        ));

        logs.push(format!(
            "Button Chord - A+B: {:?}",
            action_state.button_value(&Action::ButtonChordAB),
        ));
        logs.push(format!(
            "               Ctrl+A: {:?}",
            action_state.button_value(&Action::ButtonChordCtrlA)
        ));

        logs.push(format!(
            "DualAxis Chord - Mouse Left + Motion: {}",
            action_state
                .try_dual_axis_value(&Action::DualAxisChordMouseLeft)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string()),
        ));
        logs.push(format!(
            "                 Ctrl-A + Mouse Position: {}",
            action_state
                .try_dual_axis_value(&Action::DualAxisChordCtrlAMousePosition)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "None".to_string())
        ));

        text.0 = logs.join("\n");
    }
}
