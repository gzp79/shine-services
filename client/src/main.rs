mod avatar;
mod camera;
mod debug_functions;
mod game;
mod hud;
mod world;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn main() {
    use shine_game::app::{
        init_application,
        platform::{start_game, Config},
    };

    init_application(game::TheGame);
    start_game(Config::default());
}

#[cfg(target_family = "wasm")]
pub fn main() {
    use shine_game::app::init_application;

    init_application(game::TheGame);
}
