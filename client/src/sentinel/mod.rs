use crate::{DebugState, GameState};
use bevy::{
    app::{App, Plugin, Update},
    ecs::schedule::IntoScheduleConfigs,
    state::{
        condition::in_state,
        state::{OnEnter, OnExit},
    },
};
use shine_game::input_manager::InputManagerPlugin;

mod camera;
mod debug_action;
mod debug_camera;
mod sentinel;

pub use self::{
    debug_action::DebugAction,
    sentinel::{Sentinel, SentinelAction, SentinelConfig},
};

pub struct SentinelPlugin;

impl Plugin for SentinelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<SentinelAction>::default());

        app.insert_resource(SentinelConfig { speed: 5.0 });

        app.add_systems(
            OnEnter(GameState::InWorld),
            (sentinel::spawn_sentinel, sentinel::spawn_sentinel_debug, camera::spawn).chain(),
        );
        app.add_systems(
            Update,
            ((
                (sentinel::move_sentinel, camera::follow_sentinel).chain(),
                sentinel::update_debug,
            ))
                .run_if(in_state(GameState::InWorld)),
        );

        app.add_plugins(InputManagerPlugin::<DebugAction>::default());
        app.add_systems(OnEnter(DebugState::HasFreeCamera), debug_camera::enable);
        app.add_systems(OnExit(DebugState::HasFreeCamera), debug_camera::disable);
        app.add_systems(
            Update,
            debug_camera::handle_input.run_if(in_state(DebugState::HasFreeCamera)),
        );
    }
}
