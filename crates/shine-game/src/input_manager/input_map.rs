use crate::input_manager::{
    ActionLike, ActionState, AxisCompose, AxisLike, ButtonCompose, ButtonLike, DualAxisCompose, DualAxisLike,
    InputKind, InputSources,
};
use bevy::{
    ecs::{
        component::Component,
        system::{Query, Res},
    },
    math::Vec2,
    time::Time,
};
use std::collections::{hash_map::Entry, HashMap};

#[derive(Component)]
#[require(ActionState<A>)]
pub struct InputMap<A>
where
    A: ActionLike,
{
    enabled: bool,
    buttons: HashMap<A, (Option<Box<dyn ButtonLike>>, Option<bool>)>,
    axes: HashMap<A, (Option<Box<dyn AxisLike>>, Option<f32>)>,
    dual_axes: HashMap<A, (Option<Box<dyn DualAxisLike>>, Option<Vec2>)>,
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

    pub fn button(&self, action: &A) -> Option<&Box<dyn ButtonLike>> {
        self.buttons.get(action).and_then(|(input, _)| input.as_ref())
    }

    pub fn add_button(&mut self, action: A, input: impl ButtonLike) -> &mut Self {
        if !matches!(self.kind(&action), InputKind::Button | InputKind::None) {
            panic!("Action is already bound to a different input type");
        }

        match self.buttons.entry(action) {
            Entry::Occupied(occupied_entry) => {
                let process = occupied_entry.into_mut();
                let btn = process.0.take().unwrap();
                let btn: Box<dyn ButtonLike> = Box::new(btn.or(input));
                process.0 = Some(btn);
            }
            Entry::Vacant(vacant_entry) => {
                let btn: Box<dyn ButtonLike> = Box::new(input);
                vacant_entry.insert((Some(btn), None));
            }
        }
        self
    }

    #[must_use]
    #[inline(always)]
    pub fn with_button(mut self, action: A, input: impl ButtonLike) -> Self {
        self.add_button(action, input);
        self
    }

    pub fn axis(&self, action: &A) -> Option<&Box<dyn AxisLike>> {
        self.axes.get(action).and_then(|(input, _)| input.as_ref())
    }

    pub fn add_axis(&mut self, action: A, input: impl AxisLike) -> &mut Self {
        if !matches!(self.kind(&action), InputKind::Axis | InputKind::None) {
            panic!("Action is already bound to a different input type");
        }

        match self.axes.entry(action) {
            Entry::Occupied(occupied_entry) => {
                let process = occupied_entry.into_mut();
                let axis = process.0.take().unwrap();
                let axis: Box<dyn AxisLike> = Box::new(axis.max(input));
                process.0 = Some(axis);
            }
            Entry::Vacant(vacant_entry) => {
                let axis: Box<dyn AxisLike> = Box::new(input);
                vacant_entry.insert((Some(axis), None));
            }
        }
        self
    }

    #[must_use]
    #[inline(always)]
    pub fn with_axis(mut self, action: A, input: impl AxisLike) -> Self {
        self.add_axis(action, input);
        self
    }

    pub fn dual_axis(&self, action: &A) -> Option<&Box<dyn DualAxisLike>> {
        self.dual_axes.get(action).and_then(|(input, _)| input.as_ref())
    }

    pub fn add_dual_axis(&mut self, action: A, input: impl DualAxisLike) -> &mut Self {
        if !matches!(self.kind(&action), InputKind::DualAxis | InputKind::None) {
            panic!("Action is already bound to a different input type");
        }

        match self.dual_axes.entry(action) {
            Entry::Occupied(occupied_entry) => {
                let process = occupied_entry.into_mut();
                let dual_axis = process.0.take().unwrap();
                let axis: Box<dyn DualAxisLike> = Box::new(dual_axis.max(input));
                process.0 = Some(axis);
            }
            Entry::Vacant(vacant_entry) => {
                let axis: Box<dyn DualAxisLike> = Box::new(input);
                vacant_entry.insert((Some(axis), None));
            }
        }
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
        for input in self.buttons.values_mut() {
            if let (Some(input), _) = input {
                input.integrate(&input_source);
            }
        }

        for input in self.axes.values_mut() {
            if let (Some(input), _) = input {
                input.integrate(&input_source);
            }
        }

        for input in self.dual_axes.values_mut() {
            if let (Some(input), _) = input {
                input.integrate(&input_source);
            }
        }
    }

    pub fn process(&mut self, time: &Time) {
        for input in self.buttons.values_mut() {
            if let (Some(input), value) = input {
                *value = input.process(time);
            }
        }

        for input in self.axes.values_mut() {
            if let (Some(input), value) = input {
                *value = input.process(time);
            }
        }

        for input in self.dual_axes.values_mut() {
            if let (Some(input), value) = input {
                *value = input.process(time);
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

        for (action, input) in &input_map.buttons {
            let button_state = action_state.set_button(action.clone());
            button_state.update(input.1, time.elapsed().as_secs_f32());
        }

        for (action, input) in &input_map.axes {
            let axis_state = action_state.set_axis(action.clone());
            axis_state.value = input.1;
        }

        for (action, input) in &input_map.dual_axes {
            let dual_axis_state = action_state.set_dual_axis(action.clone());
            dual_axis_state.value = input.1;
        }

        action_state.finish_update();
    }
}
