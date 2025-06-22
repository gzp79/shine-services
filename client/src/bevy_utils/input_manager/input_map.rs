use crate::bevy_utils::input_manager::{
    ActionLike, ActionState,  AnyInputSource, AxisLike, ButtonLike, DualAxisLike, InputSource,
};
use bevy::{
    ecs::{
        resource::Resource,
        system::{Res, ResMut},
    },
    math::Vec2,
    time::Time,
};
use std::collections::HashMap;

/// A trait for inputs that can be integrated into the input map.
pub trait IntegratedInput {
    /// Integrate the input into the input map.
    fn integrate(&mut self, input: &dyn AnyInputSource, time: &Time);
}

pub trait IntegratedButtonLike: ButtonLike + IntegratedInput {}
pub trait IntegratedAxisLike: AxisLike + IntegratedInput {}
pub trait IntegratedDualAxisLike: DualAxisLike + IntegratedInput {}

#[derive(Resource)]
pub struct InputMap<A>
where
    A: ActionLike,
{
    enabled: bool,
    buttons: HashMap<A, Vec<Box<dyn IntegratedButtonLike>>>,
    axes: HashMap<A, Vec<Box<dyn IntegratedAxisLike>>>,
    dual_axes: HashMap<A, Vec<Box<dyn IntegratedDualAxisLike>>>,
}

impl<A> Default for InputMap<A>
where
    A: ActionLike,
{
    fn default() -> Self {
        Self {
            enabled: true,
            buttons: HashMap::new(),
            axes: HashMap::new(),
            dual_axes: HashMap::new(),
        }
    }
}

impl<A> InputMap<A>
where
    A: ActionLike,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn bind_button(&mut self, action: A, input: impl IntegratedButtonLike) {
        if self.axes.contains_key(&action) || self.dual_axes.contains_key(&action) {
            panic!("Action is already bound to a different input type");
        }
        self.buttons.entry(action).or_default().push(Box::new(input));
    }

    pub fn bind_axis(&mut self, action: A, input: impl IntegratedAxisLike) {
        if self.buttons.contains_key(&action) || self.dual_axes.contains_key(&action) {
            panic!("Action is already bound to a different input type");
        }
        self.axes.entry(action).or_default().push(Box::new(input));
    }

    pub fn bind_dual_axis(&mut self, action: A, input: impl IntegratedDualAxisLike) {
        if self.buttons.contains_key(&action) || self.axes.contains_key(&action) {
            panic!("Action is already bound to a different input type");
        }
        self.dual_axes.entry(action).or_default().push(Box::new(input));
    }

    pub fn integrate(&mut self, provider: &dyn AnyInputSource, time: &Time) {
        for inputs in self.buttons.values_mut() {
            for input in inputs {
                input.integrate(provider, time);
            }
        }

        for inputs in self.axes.values_mut() {
            for input in inputs {
                input.integrate(provider, time);
            }
        }

        for inputs in self.dual_axes.values_mut() {
            for input in inputs {
                input.integrate(provider, time);
            }
        }
    }
}

/// Update the action state based on the input map.
pub fn update_action_state<A>(input_map: Res<InputMap<A>>, mut action_state: ResMut<ActionState<A>>, time: Res<Time>)
where
    A: ActionLike,
{
    if !input_map.enabled {
        action_state.clear();
        return;
    }

    for (action, inputs) in &input_map.buttons {
        let button_state = action_state.button_data_mut(action);

        let mut pressed = false;
        for button_like in inputs {
            if button_like.is_down() {
                pressed = true;
                break;
            }
        }

        if button_state.pressed != pressed {
            button_state.start_time = time.elapsed().as_secs_f32();
        }
        button_state.pressed = pressed;
    }

    for (action, inputs) in &input_map.axes {
        let axis_state = action_state.axis_data_mut(action);
        let max_value = inputs
            .iter()
            .map(|a| a.value())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);
        axis_state.value = max_value;
    }

    for (action, inputs) in &input_map.dual_axes {
        let dual_axis_state = action_state.dual_axis_data_mut(action);
        let max_value = inputs
            .iter()
            .map(|a| a.value_pair())
            .max_by(|a, b| a.length_squared().partial_cmp(&b.length_squared()).unwrap())
            .unwrap_or(Vec2::ZERO);
        dual_axis_state.value = max_value;
    }
}
