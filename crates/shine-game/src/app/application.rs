use crate::app::{CameraSimulate, GameSystems};
use bevy::{
    app::{App, Update},
    asset::{AssetMetaCheck, AssetMode, AssetPlugin, UnapprovedPathMode},
    ecs::schedule::IntoScheduleConfigs,
    log,
    utils::default,
};
use std::sync::OnceLock;

/// The setup function for the application.
type SetupFn = fn(&mut App, &platform::Config);
static SETUP_FN: OnceLock<SetupFn> = OnceLock::new();

/// This function is called by the main application to initialize the application setup.
pub fn init_application(setup_fn: SetupFn) {
    if SETUP_FN.set(setup_fn).is_err() {
        log::warn!("The application setup function has already been initialized.");
    }
}

pub trait PlatformInit {
    fn platform_init(&mut self, config: &platform::Config);
}

/// Platform-specific initialization.
#[cfg(target_arch = "wasm32")]
pub mod platform {
    use super::{create_application, customized_asset_plugin, PlatformInit};
    use bevy::{
        app::{App, AppExit, PluginGroup, PostUpdate},
        ecs::event::EventWriter,
        log,
        utils::default,
        window::{Window, WindowPlugin},
        DefaultPlugins,
    };
    use std::sync::atomic::{self, AtomicBool};
    use wasm_bindgen::prelude::*;

    pub struct Config {
        pub canvas: String,
    }

    impl PlatformInit for App {
        fn platform_init(&mut self, config: &Config) {
            let Config { canvas } = config;

            self.add_plugins(
                DefaultPlugins
                    .set(WindowPlugin {
                        primary_window: Some(Window {
                            canvas: Some(canvas.clone()),
                            fit_canvas_to_parent: true,
                            ..default()
                        }),
                        ..default()
                    })
                    .set(customized_asset_plugin()),
            );
            log::info!("Initializing game for canvas: {}", canvas);
        }
    }

    static IS_APPLICATION: AtomicBool = AtomicBool::new(false);
    static EXIT_APPLICATION: AtomicBool = AtomicBool::new(false);

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
    use super::{customized_asset_plugin, PlatformInit};
    use bevy::{
        app::{App, PluginGroup},
        utils::default,
        window::{MonitorSelection, Window, WindowPlugin, WindowPosition},
        DefaultPlugins,
    };

    /// Platform-specific configuration.
    #[derive(Default)]
    pub struct Config {}

    /// Initializes platform-specific plugins.
    impl PlatformInit for App {
        fn platform_init(&mut self, _config: &Config) {
            self.add_plugins(
                DefaultPlugins
                    .set(WindowPlugin {
                        primary_window: Some(Window {
                            position: WindowPosition::Centered(MonitorSelection::Primary),
                            ..default()
                        }),
                        ..default()
                    })
                    .set(customized_asset_plugin()),
            );
        }
    }
}

fn customized_asset_plugin() -> AssetPlugin {
    AssetPlugin {
        mode: AssetMode::Unprocessed,
        meta_check: AssetMetaCheck::Never,
        unapproved_path_mode: UnapprovedPathMode::Forbid,
        watch_for_changes_override: Some(false),
        ..default()
    }
}

/// Creates a Bevy application with common setup and allows for customization.
pub fn create_application(config: platform::Config) -> App {
    let Some(setup_fn) = SETUP_FN.get() else {
        panic!("The application setup function has not been initialized. Call `init_application` first.");
    };

    let mut app = App::new();

    (setup_fn)(&mut app, &config);

    #[cfg(feature = "dev_tools")]
    {
        app.add_plugins(bevy_dev_tools::fps_overlay::FpsOverlayPlugin::default());
    }

    app.configure_sets(
        Update,
        (
            GameSystems::Action,
            GameSystems::PrepareSimulate,
            GameSystems::Simulate,
            GameSystems::PrepareRender,
        )
            .chain(),
    );

    app.configure_sets(
        Update,
        (
            CameraSimulate::PreparePose,
            CameraSimulate::SimulatePose,
            CameraSimulate::WithPose,
        )
            .chain()
            .in_set(GameSystems::PrepareSimulate),
    );

    /*#[cfg(feature = "dev_tools")]
    {
        bevy_mod_debugdump::print_schedule_graph(&mut app, bevy::app::PreUpdate);
        bevy_mod_debugdump::print_schedule_graph(&mut app, bevy::app::Update);
        bevy_mod_debugdump::print_render_graph(&mut app);
    }*/

    app
}
