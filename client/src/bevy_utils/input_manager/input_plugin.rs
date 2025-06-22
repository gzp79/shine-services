use crate::bevy_utils::input_manager::{
    action_state::ActionState,
    input_map::{update_action_state, InputMap},
    input_source::integrate_default_inputs,
    ActionLike,
};
use bevy::ecs::schedule::SystemSet;
use bevy::{
    app::{App, Plugin, PreUpdate},
    ecs::schedule::IntoScheduleConfigs,
};
use std::marker::PhantomData;

#[derive(SystemSet, Clone, Hash, Debug, PartialEq, Eq)]
pub enum InputManagerSystem {
    Integrate,
    UpdateActionState,
}

pub struct InputManagerPlugin<A: ActionLike> {
    _phantom: PhantomData<A>,
}

impl<A: ActionLike> Default for InputManagerPlugin<A> {
    fn default() -> Self {
        Self { _phantom: PhantomData }
    }
}

impl<A: ActionLike> Plugin for InputManagerPlugin<A> {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputMap<A>>()
            .init_resource::<ActionState<A>>()
            .configure_sets(
                PreUpdate,
                (InputManagerSystem::Integrate, InputManagerSystem::UpdateActionState).chain(),
            )
            .add_systems(
                PreUpdate,
                (
                    integrate_default_inputs::<A>.in_set(InputManagerSystem::Integrate),
                    update_action_state::<A>.in_set(InputManagerSystem::UpdateActionState),
                ),
            );
    }
}
