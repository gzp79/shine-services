#![cfg(target_os = "android")]

use bevy::prelude::bevy_main;

#[cfg(all(feature = "example", feature = "example_curve"))]
#[path = "../examples/curve.rs"]
mod app;

#[cfg(all(feature = "example", feature = "example_camera_orbit"))]
#[path = "../examples/camera_orbit.rs"]
mod app;

#[cfg(all(feature = "example", feature = "example_camera_follow"))]
#[path = "../examples/camera_follow.rs"]
mod app;

#[cfg(all(feature = "example", feature = "example_camera_free"))]
#[path = "../examples/camera_free.rs"]
mod app;

#[cfg(all(feature = "example", feature = "example_camera_look_at"))]
#[path = "../examples/camera_look_at.rs"]
mod app;

#[cfg(all(feature = "example", feature = "example_input_drivers"))]
#[path = "../examples/input_drivers.rs"]
mod app;

#[cfg(all(feature = "example", feature = "example_input_process"))]
#[path = "../examples/input_process.rs"]
mod app;

#[cfg(all(feature = "example", feature = "example_input_gesture"))]
#[path = "../examples/input_gesture.rs"]
mod app;

#[cfg(all(feature = "example", feature = "example_input_multiplayer"))]
#[path = "../examples/input_multiplayer.rs"]
mod app;

#[cfg(all(feature = "example", feature = "example_pinch_zoom"))]
#[path = "../examples/pinch_zoom.rs"]
mod app;

#[cfg(all(feature = "example", feature = "example_asset"))]
#[path = "../examples/asset.rs"]
mod app;

macro_rules! non_example_mods {
    ($($mod_name:ident),* $(,)?) => {
        $(
            #[cfg(not(feature = "example"))]
            mod $mod_name;
        )*
    };
}

non_example_mods! {
    avatar,
    camera,
    debug_functions,
    game,
    hud,
    world,
}

#[cfg(not(feature = "example"))]
mod app {
    use super::*;

    pub fn android_main() {
        use shine_game::app::{
            init_application,
            platform::{start_game, Config},
        };

        init_application(game::TheGame);
        start_game(Config::default());
    }
}

#[bevy_main]
fn main() {
    app::android_main();
}
