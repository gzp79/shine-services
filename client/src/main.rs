use bevy::{
    app::{App, AppExit, Update},
    ecs::{
        event::EventWriter,
        system::{Res, ResMut},
    },
    input::{keyboard::KeyCode, ButtonInput},
    state::{
        app::AppExtStates,
        state::{NextState, State, States},
    },
};

use crate::{camera::CameraPlugin, world::WorldPlugin};

mod camera;
mod camera_rig;
mod world;

mod poc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum GameState {
    //MainMenu,
    Playing,

    //POCs
    CameraOrbitPOC,
    CameraFreePOC,
    CameraLookAtPOC,
    CameraFollowPOC,
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::create_application;
    use bevy::{
        app::{App, AppExit, PluginGroup, PostUpdate},
        ecs::event::EventWriter,
        utils::default,
        window::{Window, WindowPlugin},
        DefaultPlugins,
    };
    use std::sync::atomic::{self, AtomicBool};
    use wasm_bindgen::prelude::*;

    static IS_APPLICATION: AtomicBool = AtomicBool::new(false);
    static EXIT_APPLICATION: AtomicBool = AtomicBool::new(false);

    pub struct Config {
        canvas: String,
    }

    pub fn platform_init(app: &mut App, config: Config) {
        let Config { canvas } = config;

        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some(canvas.clone()),
                ..default()
            }),
            ..default()
        }));
        app.add_systems(PostUpdate, exit_system);

        log::info!("Initializing game for canvas: {}", canvas);
    }

    fn exit_system(mut exit: EventWriter<AppExit>) {
        if EXIT_APPLICATION.load(atomic::Ordering::SeqCst) {
            log::info!("Exiting application...");
            exit.write(AppExit::Success);
        }
    }

    #[wasm_bindgen]
    pub fn start_game(canvas: String) {
        if IS_APPLICATION
            .compare_exchange(false, true, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst)
            .is_err()
        {
            log::error!("Game is already running.");
            return;
        }

        log::info!("Starting game...");
        create_application(Config { canvas });
    }

    #[wasm_bindgen]
    pub fn stop_game() {
        log::info!("Stopping game...");
        EXIT_APPLICATION.store(true, atomic::Ordering::SeqCst);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use bevy::{app::App, DefaultPlugins};

    pub struct Config;

    pub fn platform_init(app: &mut App, _config: Config) {
        app.add_plugins(DefaultPlugins);
    }
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    /* in wasm the application is created in the start_game to handle  */
}

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    create_application(platform::Config);
}

fn create_application(config: platform::Config) {
    let mut app = App::new();
    platform::platform_init(&mut app, config);

    app.insert_state(GameState::CameraOrbitPOC);

    app.add_plugins(CameraPlugin { state: GameState::Playing });
    app.add_plugins(WorldPlugin { state: GameState::Playing });

    app.add_plugins(poc::CameraOrbitPOC {
        state: GameState::CameraOrbitPOC,
    });
    app.add_plugins(poc::CameraFreePOC {
        state: GameState::CameraFreePOC,
    });
    app.add_plugins(poc::CameraLookAtPOC {
        state: GameState::CameraLookAtPOC,
    });
    app.add_plugins(poc::CameraFollowPOC {
        state: GameState::CameraFollowPOC,
    });
    app.add_systems(Update, next_poc);

    app.run();
}

fn next_poc(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameState>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut exit: EventWriter<AppExit>,
) {
    if keyboard_input.just_pressed(KeyCode::Tab) {
        match game_state.get() {
            GameState::CameraOrbitPOC => next_game_state.set(GameState::CameraFreePOC),
            GameState::CameraFreePOC => next_game_state.set(GameState::CameraLookAtPOC),
            GameState::CameraLookAtPOC => next_game_state.set(GameState::CameraFollowPOC),
            GameState::CameraFollowPOC => next_game_state.set(GameState::CameraOrbitPOC),
            _ => {}
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    if keyboard_input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
