use bevy::{
    app::App,
    tasks::{AsyncComputeTaskPool, TaskPool},
    DefaultPlugins,
};
use godot::classes::{INode, Node};
use godot::prelude::*;
use std::sync::Once;

static INIT: Once = Once::new();

pub fn init_bevy_once() {
    INIT.call_once(|| {
        log::info!("Initializing TaskPool...");
        AsyncComputeTaskPool::get_or_init(TaskPool::new);
    });
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct BevyApp {
    app: Option<App>,
}

impl BevyApp {}

#[godot_api]
impl INode for BevyApp {
    fn init(_base: Base<Node>) -> Self {
        Self { app: None }
    }

    fn ready(&mut self) {
        log::info!("Creating Bevy app...");

        init_bevy_once();

        let mut app = App::new();
        app.add_plugins(DefaultPlugins);

        log::info!("Bevy app created");
        self.app = Some(app);
    }
}
