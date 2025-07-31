use crate::input_manager::{ActionLike, ActionState, ActionStates, InputSources, TypedUserInput, UserInput};
use bevy::{
    ecs::{
        component::Component,
        system::{Query, Res},
    },
    time::Time,
};
use std::collections::HashMap;

pub trait IntoActionState: Send + Sync + 'static {
    type State: ActionState + Default;

    fn update_action_state(&self, state: &mut Self::State, time_s: f32);
}

trait InputPipeline<A>: Send + Sync + 'static
where
    A: ActionLike,
{
    fn input(&self) -> &dyn UserInput;

    fn integrate(&mut self, input_source: &InputSources);
    fn pull_pipeline(&mut self, time_s: f32);
    fn store_state(&self, action_state: &mut ActionStates<A>, action: &A);
}

/// Store the result of an input processing pipeline and converts it into an `ActionState`.
struct StoredInput<T, I>
where
    T: IntoActionState,
    I: TypedUserInput<T>,
{
    input: I,
    /// the last pull time
    time_s: f32,
    /// the last pulled value
    value: Option<T>,
}

impl<T, I> StoredInput<T, I>
where
    T: IntoActionState,
    I: TypedUserInput<T>,
{
    pub fn new(input: I) -> Self {
        Self {
            input,
            time_s: 0.0,
            value: None,
        }
    }
}

impl<A, T, I> InputPipeline<A> for StoredInput<T, I>
where
    A: ActionLike,
    T: IntoActionState,
    I: TypedUserInput<T>,
{
    fn input(&self) -> &dyn UserInput {
        &self.input
    }

    fn integrate(&mut self, input_source: &InputSources) {
        self.input.integrate(input_source);
    }

    fn pull_pipeline(&mut self, time_s: f32) {
        self.time_s = time_s;
        self.value = self.input.process(time_s);
    }

    fn store_state(&self, action_state: &mut ActionStates<A>, action: &A) {
        if let Some(value) = &self.value {
            let state = action_state.set_as::<T::State>(action.clone());
            value.update_action_state(state, self.time_s);
        } else {
            action_state.remove(action);
        }
    }
}

#[derive(Component)]
#[require(ActionStates<A>)]
pub struct InputMap<A>
where
    A: ActionLike,
{
    enabled: bool,
    bindings: HashMap<A, Box<dyn InputPipeline<A>>>,
}

impl<A> Default for InputMap<A>
where
    A: ActionLike,
{
    fn default() -> Self {
        Self {
            enabled: true,
            bindings: HashMap::new(),
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

    pub fn bind<T, I>(&mut self, action: &A, input: I)
    where
        I: TypedUserInput<T>,
        T: IntoActionState,
    {
        let input = Box::new(StoredInput::new(input));
        self.bindings.insert(action.clone(), input);
    }

    pub fn get(&self, action: &A) -> Option<&dyn UserInput> {
        self.bindings.get(action).map(|pipeline| pipeline.input())
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn integrate(&mut self, input_source: &InputSources) {
        for input in self.bindings.values_mut() {
            input.integrate(input_source);
        }
    }

    pub fn process(&mut self, time_s: f32) {
        for input in self.bindings.values_mut() {
            input.pull_pipeline(time_s);
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

        input_map.process(time.elapsed_secs());
    }
}

/// Update the action state based on the input map.
pub fn update_action_state<A>(mut input_query: Query<(&InputMap<A>, &mut ActionStates<A>)>)
where
    A: ActionLike,
{
    for (input_map, mut action_state) in input_query.iter_mut() {
        if !input_map.enabled {
            action_state.clear();
            return;
        }

        action_state.start_update();

        for (action, input) in &input_map.bindings {
            input.store_state(&mut action_state, action);
        }

        action_state.finish_update();
    }
}
