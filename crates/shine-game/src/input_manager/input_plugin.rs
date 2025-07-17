use crate::input_manager::{
    integrate_gamepad_inputs, integrate_keyboard_inputs, integrate_mouse_inputs, integrate_touch_inputs,
    integrate_two_finger_touch_inputs, process_inputs, update_action_state, update_two_finger_touch_gesture,
    ActionLike, GamepadManager, PinchGestureState,
};
use bevy::{
    app::{App, Plugin, PreUpdate},
    ecs::schedule::{IntoScheduleConfigs, SystemSet},
    input::InputSystem,
};
use std::marker::PhantomData;

#[derive(SystemSet, Clone, Hash, Debug, PartialEq, Eq)]
pub enum InputManagerSystem {
    SourceInput,
    Integrate,
    Process,
    UpdateActionState,
}

struct InputManagerCommonPlugin;

impl Plugin for InputManagerCommonPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GamepadManager);
        app.insert_resource(PinchGestureState::default());

        app.configure_sets(
            PreUpdate,
            (
                InputManagerSystem::SourceInput,
                InputManagerSystem::Integrate,
                InputManagerSystem::Process,
                InputManagerSystem::UpdateActionState,
            )
                .chain()
                .after(InputSystem),
        );

        app.add_systems(
            PreUpdate,
            update_two_finger_touch_gesture.in_set(InputManagerSystem::SourceInput),
        );
    }
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
        if !app.is_plugin_added::<InputManagerCommonPlugin>() {
            app.add_plugins(InputManagerCommonPlugin);
        }

        app.add_systems(
            PreUpdate,
            (
                integrate_keyboard_inputs::<A>,
                integrate_mouse_inputs::<A>,
                integrate_touch_inputs::<A>,
                integrate_two_finger_touch_inputs::<A>,
                integrate_gamepad_inputs::<A>,
            )
                .in_set(InputManagerSystem::Integrate),
        );

        app.add_systems(PreUpdate, process_inputs::<A>.in_set(InputManagerSystem::Process));

        app.add_systems(
            PreUpdate,
            update_action_state::<A>.in_set(InputManagerSystem::UpdateActionState),
        );
    }
}
