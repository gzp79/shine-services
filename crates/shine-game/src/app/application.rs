use crate::app::{CameraSimulate, GameSystems};
use bevy::{
    app::{App, Update},
    asset::{AssetMetaCheck, AssetMode, AssetPlugin, UnapprovedPathMode},
    ecs::schedule::IntoScheduleConfigs,
    log,
    tasks::BoxedFuture,
    utils::default,
};
use std::{any::Any, sync::OnceLock};

pub trait GameSetup: Send + Sync + 'static {
    type GameConfig: Any + Send + Sync + 'static;

    /// Asynchronously create the game configuration based on the platform configuration.
    fn create_setup<'a>(&'a self, config: &'a platform::Config) -> BoxedFuture<'a, Self::GameConfig>;

    /// Set up the application with the provided platform and game configurations.
    fn setup_application(&self, app: &mut App, platform_config: &platform::Config, game_config: Self::GameConfig);
}

/// This function is called by the main application to initialize the application setup.
pub fn init_application<G>(game_setup: G)
where
    G: GameSetup,
{
    if GAME_SETUP.set(Box::new(game_setup)).is_err() {
        log::warn!("The application setup function has already been initialized.");
    }
}

trait ErasedGameSetup: Send + Sync + 'static {
    fn create_setup<'a>(&'a self, config: &'a platform::Config) -> BoxedFuture<'a, Box<dyn Any + Send + Sync>>;

    fn setup_application(
        &self,
        app: &mut App,
        platform_config: &platform::Config,
        game_config: Box<dyn Any + Send + Sync>,
    );
}

impl<T> ErasedGameSetup for T
where
    T: GameSetup,
{
    fn create_setup<'a>(&'a self, config: &'a platform::Config) -> BoxedFuture<'a, Box<dyn Any + Send + Sync>> {
        Box::pin(async move {
            let app_config: Box<dyn Any + Send + Sync> =
                Box::new(<Self as GameSetup>::create_setup(self, config).await);
            app_config
        })
    }

    fn setup_application(
        &self,
        app: &mut App,
        platform_config: &platform::Config,
        game_config: Box<dyn Any + Send + Sync>,
    ) {
        let game_config = game_config
            .downcast::<<Self as GameSetup>::GameConfig>()
            .expect("Failed to downcast game configuration");
        self.setup_application(app, platform_config, *game_config);
    }
}

static GAME_SETUP: OnceLock<Box<dyn ErasedGameSetup>> = OnceLock::new();

pub trait PlatformInit {
    fn platform_init(&mut self, config: &platform::Config);
}

/// Platform-specific initialization.
#[cfg(target_family = "wasm")]
pub mod platform {
    use super::{customized_asset_plugin, setup_application_common, PlatformInit, GAME_SETUP};
    use bevy::{
        app::{App, AppExit, PluginGroup, PostUpdate},
        ecs::message::MessageWriter,
        log,
        utils::default,
        window::{Window, WindowPlugin},
        DefaultPlugins,
    };
    use std::{
        any::Any,
        mem,
        sync::{
            atomic::{self, AtomicBool},
            Mutex,
        },
    };
    use wasm_bindgen::prelude::*;

    pub struct Config {
        pub canvas: String,
        pub enable_dev_tools: bool,
        pub show_schedule_graphs: bool,
    }

    impl Config {
        fn new(canvas: String) -> Self {
            Self {
                canvas,
                enable_dev_tools: false,
                show_schedule_graphs: false,
            }
        }
    }

    impl PlatformInit for App {
        fn platform_init(&mut self, config: &Config) {
            let Config { canvas, .. } = config;

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

    enum AppState {
        None,
        Created(Config, Box<dyn Any + Send + Sync>),
        Running,
        InProcess,
    }

    static APPLICATION: Mutex<AppState> = Mutex::new(AppState::None);
    static EXIT_APPLICATION: AtomicBool = AtomicBool::new(false);

    fn exit_system(mut exit: MessageWriter<AppExit>) {
        if EXIT_APPLICATION.load(atomic::Ordering::SeqCst) {
            log::info!("Exiting application...");
            exit.write(AppExit::Success);
        }
    }

    #[wasm_bindgen]
    pub async fn create_game(canvas: String) {
        let game_setup = GAME_SETUP
            .get()
            .expect("The application setup function has not been initialized. Call init_application first.");

        let mut current_app = APPLICATION.lock().unwrap();

        match mem::replace(&mut *current_app, AppState::InProcess) {
            AppState::None => {}
            AppState::Created(..) => {
                log::warn!("Game is re-created without running");
            }
            AppState::Running => {
                *current_app = AppState::Running;
                log::error!("Game is already running, stop it first.");
                return;
            }
            AppState::InProcess => {
                unimplemented!("InProcess state should be valid only temporarily in a critical section")
            }
        }

        log::info!("Creating game...");

        let platform_config = Config::new(canvas);
        let game_config: Box<dyn Any + Send + Sync> = game_setup.create_setup(&platform_config).await;
        *current_app = AppState::Created(platform_config, game_config);
    }

    #[wasm_bindgen]
    pub fn start_game() {
        let game_setup = GAME_SETUP
            .get()
            .expect("The application setup function has not been initialized. Call init_application first.");

        let (platform_config, game_config) = {
            let mut current_app = APPLICATION.lock().unwrap();
            match mem::replace(&mut *current_app, AppState::InProcess) {
                AppState::Created(platform_config, game_config) => {
                    *current_app = AppState::Running;
                    (platform_config, game_config)
                }
                AppState::Running => {
                    *current_app = AppState::Running;
                    log::error!("Game is already running, stop it first.");
                    return;
                }
                AppState::None => {
                    log::error!("Game has not been created. Call start_game first.");
                    return;
                }
                AppState::InProcess => {
                    unimplemented!("InProcess state should be valid only temporarily in a critical section")
                }
            }
        };

        let mut app = App::new();

        game_setup.setup_application(&mut app, &platform_config, game_config);
        app.add_systems(PostUpdate, exit_system);
        setup_application_common(&mut app, &platform_config);

        app.run();

        log::info!("Application stopped...");
        {
            let mut current_app = APPLICATION.lock().unwrap();
            log::info!("Application is release...");
            *current_app = AppState::None;
        }
    }

    #[wasm_bindgen]
    pub fn stop_game() {
        log::info!("Stopping game...");
        EXIT_APPLICATION.store(true, atomic::Ordering::SeqCst);
    }
}

/// Platform-specific initialization.
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "android"
))]
pub mod platform {
    use super::{customized_asset_plugin, setup_application_common, PlatformInit, GAME_SETUP};
    use bevy::{
        app::{App, PluginGroup},
        tasks::block_on,
        utils::default,
        window::{MonitorSelection, Window, WindowPlugin, WindowPosition},
        DefaultPlugins,
    };

    /// Platform-specific configuration.
    pub struct Config {
        pub enable_dev_tools: bool,
        pub show_schedule_graphs: bool,
    }

    #[allow(clippy::derivable_impls)]
    impl Default for Config {
        fn default() -> Self {
            Self {
                enable_dev_tools: false,
                show_schedule_graphs: false,
            }
        }
    }

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

    pub fn start_game(config: Config) {
        let game_setup = GAME_SETUP
            .get()
            .expect("The application setup function has not been initialized. Call init_application first.");

        let game_config = block_on(game_setup.create_setup(&config));

        let mut app = App::new();
        setup_application_common(&mut app, &config);
        game_setup.setup_application(&mut app, &config, game_config);

        app.run();
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

fn setup_application_common(app: &mut App, config: &platform::Config) {
    #[cfg(feature = "dev_tools")]
    {
        if config.enable_dev_tools {
            app.add_plugins(bevy_dev_tools::fps_overlay::FpsOverlayPlugin::default());
        }
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

    #[cfg(feature = "dev_tools")]
    {
        if config.show_schedule_graphs {
            bevy_mod_debugdump::print_schedule_graph(app, bevy::app::PreUpdate);
            bevy_mod_debugdump::print_schedule_graph(app, bevy::app::Update);
            bevy_mod_debugdump::print_render_graph(app);
        }
    }
}
