use bevy::app::App;
use std::sync::OnceLock;

/// The setup function for the application.
type SetupFn = fn(&mut App);
static SETUP_FN: OnceLock<SetupFn> = OnceLock::new();

/// This function is called by the main application to initialize the application setup.
pub fn init(setup_fn: SetupFn) {
    if SETUP_FN.set(setup_fn).is_err() {
        log::warn!("The application setup function has already been initialized.");
    }
}

/// Platform-specific initialization.
#[cfg(target_arch = "wasm32")]
pub mod platform {
    use super::{create_application, SETUP_FN};
    use bevy::{
        app::{App, AppExit, PluginGroup, PostUpdate},
        ecs::event::EventWriter,
        utils::default,
        window::{Window, WindowPlugin},
        DefaultPlugins,
    };
    use std::sync::atomic::{self, AtomicBool};
    use wasm_bindgen::prelude::*;

    pub struct Config {
        pub canvas: String,
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
        log::info!("Initializing game for canvas: {}", canvas);
    }

    static IS_APPLICATION: AtomicBool = AtomicBool::new(false);
    static EXIT_APPLICATION: AtomicBool = AtomicBool::new(false);

    fn exit_system(mut exit: EventWriter<bevy::app::AppExit>) {
        if EXIT_APPLICATION.load(atomic::Ordering::SeqCst) {
            log::info!("Exiting application...");
            exit.write(bevy::app::AppExit::Success);
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
        let mut app = create_application(Config { canvas });

        app.add_systems(PostUpdate, exit_system);
        app.run();
    }

    #[wasm_bindgen]
    pub fn stop_game() {
        log::info!("Stopping game...");
        EXIT_APPLICATION.store(true, atomic::Ordering::SeqCst);
    }
}

/// Platform-specific initialization.
#[cfg(not(target_arch = "wasm32"))]
pub mod platform {
    use bevy::{app::App, DefaultPlugins};

    /// Platform-specific configuration.
    #[derive(Default)]
    pub struct Config;

    /// Initializes platform-specific plugins.
    pub fn platform_init(app: &mut App, _config: Config) {
        app.add_plugins(DefaultPlugins);
    }
}

/// Creates a Bevy application with common setup and allows for customization.
pub fn create_application(config: platform::Config) -> App {
    let mut app = App::new();
    platform::platform_init(&mut app, config);

    if let Some(setup_fn) = SETUP_FN.get() {
        (setup_fn)(&mut app);
    } else {
        log::error!("The application setup function has not been initialized. Call `application::init` first.");
    }

    app
}
