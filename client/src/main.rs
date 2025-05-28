use bevy::app::App;

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

    app.add_plugins(bevy::dev_tools::fps_overlay::FpsOverlayPlugin::default());
    app.add_plugins(overlay::SizeOverlayPlugin);

    app.run();
}

mod overlay {
    use bevy::{
        app::{App, Plugin, Startup, Update},
        core_pipeline::core_2d::Camera2d,
        ecs::{
            component::Component,
            event::EventReader,
            query::With,
            system::{Commands, Single},
        },
        text::TextFont,
        ui::{widget::Text, Node, Val},
        utils::default,
        window::WindowResized,
    };

    #[derive(Component)]
    struct ResolutionText;

    fn setup_camera(mut commands: Commands) {
        commands.spawn(Camera2d);
    }

    fn setup_ui(mut commands: Commands) {
        commands
            .spawn(Node {
                width: Val::Percent(100.),
                top: Val::Px(30.),
                ..default()
            })
            .with_child((
                Text::new("Resolution"),
                TextFont { font_size: 42.0, ..default() },
                ResolutionText,
            ));
    }

    fn on_resize_system(
        mut text: Single<&mut Text, With<ResolutionText>>,
        mut resize_reader: EventReader<WindowResized>,
    ) {
        for e in resize_reader.read() {
            text.0 = format!("{:.1} x {:.1}", e.width, e.height);
        }
    }

    pub struct SizeOverlayPlugin;

    impl Plugin for SizeOverlayPlugin {
        fn build(&self, app: &mut App) {
            app.add_systems(Startup, (setup_camera, setup_ui))
                .add_systems(Update, on_resize_system);
        }
    }
}
