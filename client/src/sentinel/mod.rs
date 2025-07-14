use bevy::{
    app::{App, Plugin, Update},
    ecs::schedule::IntoScheduleConfigs,
    state::{
        condition::in_state,
        state::{OnEnter, OnExit, States},
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

pub struct SentinelPlugin<S: States> {
    pub state: S,
}

impl<S: States> Plugin for SentinelPlugin<S> {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<SentinelAction>::default());
        app.add_plugins(InputManagerPlugin::<DebugAction>::default());

        app.insert_resource(SentinelConfig { speed: 5.0 });

        app.add_systems(
            OnEnter(self.state.clone()),
            (sentinel::spawn_sentinel, sentinel::spawn_sentinel_debug, camera::spawn).chain(),
        )
        .add_systems(
            OnExit(self.state.clone()),
            (sentinel::despawn_sentinel, camera::despawn).chain(),
        )
        .add_systems(
            Update,
            (
                (sentinel::move_sentinel, camera::follow_sentinel).chain(),
                sentinel::update_debug,
                debug_camera::enable,
                debug_camera::disable,
                debug_camera::handle_input,
            )
                .run_if(in_state(self.state.clone())),
        );
    }
}
