use crate::core::{init_bevy_once, GodotLogger};
use godot::prelude::*;

struct ShineClient;

#[gdextension]
unsafe impl ExtensionLibrary for ShineClient {
    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Scene {
            godot_print!("Scene init...");
            GodotLogger::init();

            log::info!("Init Bevy...");
            init_bevy_once();
            log::info!("Init completed");
        }
    }
}
