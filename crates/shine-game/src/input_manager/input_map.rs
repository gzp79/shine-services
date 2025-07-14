use crate::input_manager::{ActionLike, ActionState, AxisLike, ButtonLike, DualAxisLike, InputKind, InputSources};
use bevy::{
    ecs::{
        component::Component,
        system::{Query, Res},
    },
    math::Vec2,
    time::Time,
};
use std::collections::HashMap;

#[derive(Component)]
#[require(ActionState<A>)]
pub struct InputMap<A>
where
    A: ActionLike,
{
    enabled: bool,
    buttons: HashMap<A, Vec<(Box<dyn ButtonLike>, Option<bool>)>>,
    axes: HashMap<A, Vec<(Box<dyn AxisLike>, Option<f32>)>>,
    dual_axes: HashMap<A, Vec<(Box<dyn DualAxisLike>, Option<Vec2>)>>,
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

    pub fn add_button(&mut self, action: A, input: impl ButtonLike) -> &mut Self {
        if !matches!(self.kind(&action), InputKind::Button | InputKind::None) {
            panic!("Action is already bound to a different input type");
        }

        self.buttons.entry(action).or_default().push((Box::new(input), None));
        self
    }

    #[must_use]
    #[inline(always)]
    pub fn with_button(mut self, action: A, input: impl ButtonLike) -> Self {
        self.add_button(action, input);
        self
    }

    pub fn add_axis(&mut self, action: A, input: impl AxisLike) -> &mut Self {
        if !matches!(self.kind(&action), InputKind::Axis | InputKind::None) {
            panic!("Action is already bound to a different input type");
        }

        self.axes.entry(action).or_default().push((Box::new(input), None));
        self
    }

    #[must_use]
    #[inline(always)]
    pub fn with_axis(mut self, action: A, input: impl AxisLike) -> Self {
        self.add_axis(action, input);
        self
    }

    pub fn add_dual_axis(&mut self, action: A, input: impl DualAxisLike) -> &mut Self {
        if !matches!(self.kind(&action), InputKind::DualAxis | InputKind::None) {
            panic!("Action is already bound to a different input type");
        }

        self.dual_axes.entry(action).or_default().push((Box::new(input), None));
        self
    }

    #[must_use]
    #[inline(always)]
    pub fn with_dual_axis(mut self, action: A, input: impl DualAxisLike) -> Self {
        self.add_dual_axis(action, input);
        self
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn kind(&self, action: &A) -> InputKind {
        if self.buttons.contains_key(action) {
            InputKind::Button
        } else if self.axes.contains_key(action) {
            InputKind::Axis
        } else if self.dual_axes.contains_key(action) {
            InputKind::DualAxis
        } else {
            InputKind::None
        }
    }

    pub fn integrate(&mut self, input_source: InputSources) {
        for inputs in self.buttons.values_mut() {
            for (input, _) in inputs {
                input.integrate(&input_source);
            }
        }

        for inputs in self.axes.values_mut() {
            for (input, _) in inputs {
                input.integrate(&input_source);
            }
        }

        for inputs in self.dual_axes.values_mut() {
            for (input, _) in inputs {
                input.integrate(&input_source);
            }
        }
    }

    pub fn process(&mut self, time: &Time) {
        for inputs in self.buttons.values_mut() {
            for (input, value) in inputs {
                *value = input.process(&time);
            }
        }

        for inputs in self.axes.values_mut() {
            for (input, value) in inputs {
                *value = input.process(&time);
            }
        }

        for inputs in self.dual_axes.values_mut() {
            for (input, value) in inputs {
                *value = input.process(&time);
            }
        }
    }
}

/// Update the action state based on the input map.
pub fn process_inputs<A>(mut input_query: Query<&mut InputMap<A>>, time: Res<Time>)
where
    A: ActionLike,
{
    for mut input_map in input_query.iter_mut() {
        if !input_map.enabled {
            continue;
        }

        input_map.process(&time);
    }
}

/// Update the action state based on the input map.
pub fn update_action_state<A>(mut input_query: Query<(&InputMap<A>, &mut ActionState<A>)>, time: Res<Time>)
where
    A: ActionLike,
{
    for (input_map, mut action_state) in input_query.iter_mut() {
        if !input_map.enabled {
            action_state.clear();
            return;
        }

        action_state.start_update();

        for (action, inputs) in &input_map.buttons {
            let button_state = action_state.set_button(action.clone());

            let pressed = inputs
                .iter()
                .filter_map(|(_, value)| *value)
                .max_by(|a, b| a.partial_cmp(b).unwrap());
            button_state.update(pressed, time.elapsed().as_secs_f32());
        }

        for (action, inputs) in &input_map.axes {
            let axis_state = action_state.set_axis(action.clone());

            let max_value = inputs
                .iter()
                .filter_map(|(_, v)| *v)
                .max_by(|a, b| a.partial_cmp(b).unwrap());
            axis_state.value = max_value;
        }

        for (action, inputs) in &input_map.dual_axes {
            let dual_axis_state = action_state.set_dual_axis(action.clone());

            let max_value = inputs
                .iter()
                .filter_map(|(_, v)| *v)
                .max_by(|a, b| a.length_squared().partial_cmp(&b.length_squared()).unwrap());
            dual_axis_state.value = max_value;
        }

        action_state.finish_update();
    }
}
