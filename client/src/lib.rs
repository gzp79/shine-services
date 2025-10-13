#![cfg(target_os = "android")]

use bevy::prelude::bevy_main;

#[cfg(all(feature = "example", feature = "examples_curve"))]
#[path = "../examples/curve.rs"]
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
