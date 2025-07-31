use crate::input_manager::{ActionLike, ActionStates, InputSources, IntoActionState, TypedUserInput, UserInput};
use std::{any::Any, marker::PhantomData};

pub trait InputPipeline<A>: Send + Sync + 'static
where
    A: ActionLike,
{
    fn remove_input(&mut self, id: usize);

    fn integrate(&mut self, input_source: &InputSources);
    fn pull_pipeline(&mut self, time_s: f32);
    fn store_state(&mut self, action_state: &mut ActionStates<A>, action: &A);
}

pub trait TypedInputPipeline<A, T>: InputPipeline<A>
where
    A: ActionLike,
    T: IntoActionState,
{
    fn add_input(&mut self, id: usize, input: Box<dyn TypedUserInput<T>>);
    fn get_input(&self, id: usize) -> Option<&dyn TypedUserInput<T>>;
}

pub trait AnyInputPipeline<A>: InputPipeline<A> + Any
where
    A: ActionLike,
{
    /// Returns `self` as `&dyn Any`
    fn as_any(&self) -> &dyn std::any::Any;

    /// Returns `self` as `&mut dyn Any`
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<A, T> AnyInputPipeline<A> for T
where
    A: ActionLike,
    T: InputPipeline<A> + Any,
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Store the result of an input processing pipeline and converts it into an `ActionState`.
pub struct StoredInput<A, T>
where
    A: ActionLike,
    T: IntoActionState,
{
    inputs: Vec<(usize, Box<dyn TypedUserInput<T>>, Option<T>)>,
    time_s: f32,
    _ph: PhantomData<A>,
}

impl<A, T> Default for StoredInput<A, T>
where
    A: ActionLike,
    T: IntoActionState,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A, T> StoredInput<A, T>
where
    A: ActionLike,
    T: IntoActionState,
{
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            time_s: 0.0,
            _ph: PhantomData,
        }
    }
}

impl<A, T> InputPipeline<A> for StoredInput<A, T>
where
    A: ActionLike,
    T: IntoActionState,
{
    fn remove_input(&mut self, id: usize) {
        self.inputs.retain(|(pid, _, _)| *pid != id);
    }

    fn integrate(&mut self, input_source: &InputSources) {
        for (_, input, _) in &mut self.inputs {
            input.integrate(input_source);
        }
    }

    fn pull_pipeline(&mut self, time_s: f32) {
        self.time_s = time_s;
        for (_, input, value) in &mut self.inputs {
            *value = input.process(time_s);
        }
    }

    fn store_state(&mut self, action_state: &mut ActionStates<A>, action: &A) {
        let mut cumulated_value = None;

        for (_, _, value) in self.inputs.iter_mut() {
            cumulated_value = T::accumulate(cumulated_value.take(), value.take());
        }

        let state = action_state.set_as::<T::State>(action.clone());
        T::update_state(state, cumulated_value, self.time_s);
    }
}

impl<A, T> TypedInputPipeline<A, T> for StoredInput<A, T>
where
    A: ActionLike,
    T: IntoActionState,
{
    fn add_input(&mut self, id: usize, input: Box<dyn TypedUserInput<T>>) {
        assert!(
            !self.inputs.iter().any(|(pid, _, _)| *pid == id),
            "Input with this ID already exists"
        );

        self.inputs.push((id, input, None));
    }

    fn get_input(&self, id: usize) -> Option<&dyn TypedUserInput<T>> {
        self.inputs
            .iter()
            .find(|(pid, _, _)| *pid == id)
            .map(|(_, input, _)| input.as_ref())
    }
}
