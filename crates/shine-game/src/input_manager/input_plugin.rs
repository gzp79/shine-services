use crate::input_manager::{
    detect_attached_unistroke_gesture, detect_unistroke_gesture, integrate_gamepad_inputs, integrate_gesture_inputs,
    integrate_keyboard_inputs, integrate_mouse_inputs, integrate_touch_inputs, process_inputs, update_action_state,
    update_pinch_gesture, update_pinch_gesture_emulate, ActionLike,
};
use bevy::{
    app::{App, Plugin, PreUpdate},
    ecs::schedule::{IntoScheduleConfigs, SystemSet},
    input::InputSystem,
    log,
};
use std::marker::PhantomData;

#[derive(SystemSet, Clone, Hash, Debug, PartialEq, Eq)]
pub enum InputManagerSystem {
    SourceInput,
    Integrate,
    Process,
    UpdateActions,
    ProcessActions,
}

pub struct InputManagerConfigurePlugin {
    emulate_pinch_gesture: bool,
}

impl Default for InputManagerConfigurePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl InputManagerConfigurePlugin {
    pub fn new() -> Self {
        Self { emulate_pinch_gesture: false }
    }
}

impl InputManagerConfigurePlugin {
    pub fn with_emulate_pinch_gesture(mut self, emulate: bool) -> Self {
        self.emulate_pinch_gesture = emulate;
        self
    }
}

impl Plugin for InputManagerConfigurePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PreUpdate,
            (
                InputManagerSystem::SourceInput,
                InputManagerSystem::Integrate,
                InputManagerSystem::Process,
                InputManagerSystem::UpdateActions,
                InputManagerSystem::ProcessActions,
            )
                .chain()
                .after(InputSystem),
        );

        if self.emulate_pinch_gesture {
            log::info!("Emulating pinch gesture input");
            app.add_systems(
                PreUpdate,
                update_pinch_gesture_emulate.in_set(InputManagerSystem::SourceInput),
            );
        } else {
            app.add_systems(PreUpdate, update_pinch_gesture.in_set(InputManagerSystem::SourceInput));
        }
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
        if !app.is_plugin_added::<InputManagerConfigurePlugin>() {
            app.add_plugins(InputManagerConfigurePlugin::default());
        }

        app.add_systems(
            PreUpdate,
            (
                integrate_keyboard_inputs::<A>,
                integrate_mouse_inputs::<A>,
                integrate_touch_inputs::<A>,
                integrate_gamepad_inputs::<A>,
                integrate_gesture_inputs::<A>,
            )
                .in_set(InputManagerSystem::Integrate),
        );

        app.add_systems(PreUpdate, process_inputs::<A>.in_set(InputManagerSystem::Process));

        app.add_systems(
            PreUpdate,
            update_action_state::<A>.in_set(InputManagerSystem::UpdateActions),
        );

        app.add_systems(
            PreUpdate,
            (detect_unistroke_gesture::<A>, detect_attached_unistroke_gesture::<A>)
                .in_set(InputManagerSystem::ProcessActions),
        );
    }
}
