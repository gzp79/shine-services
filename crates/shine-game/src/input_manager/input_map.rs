use crate::input_manager::{
    ActionLike, ActionState, CollectingPipeline, InputDrivers, InputError, InputPipeline, InputValueFold,
    IntoActionValue, TypedInputPipeline, TypedInputProcessor,
};
use bevy::{
    ecs::{
        component::Component,
        system::{Query, Res},
    },
    time::Time,
};
use std::{collections::HashMap, fmt, ops::DerefMut};

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct BindingId<A>(A, usize)
where
    A: ActionLike;

impl<A> fmt::Debug for BindingId<A>
where
    A: ActionLike + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}#{}", self.0, self.1)
    }
}

#[derive(Component)]
#[require(ActionState<A>)]
pub struct InputMap<A>
where
    A: ActionLike,
{
    next_id: usize,
    enabled: bool,
    bindings: HashMap<A, Box<dyn InputPipeline<A>>>,
}

impl<A> Default for InputMap<A>
where
    A: ActionLike,
{
    fn default() -> Self {
        Self {
            next_id: 1,
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

    fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Bind a new pipeline to an action and return its ID.
    pub fn bind<T, I>(&mut self, action: A, input: I) -> Result<BindingId<A>, InputError>
    where
        I: TypedInputProcessor<T>,
        T: IntoActionValue,
    {
        let id = self.next_id();
        let pipeline = self.bindings.entry(action.clone()).or_insert_with(|| {
            let pipeline: Box<dyn InputPipeline<A>> = Box::new(CollectingPipeline::<A, T>::new());
            pipeline
        });

        if let Some(pipeline) = pipeline
            .deref_mut()
            .as_any_mut()
            .downcast_mut::<CollectingPipeline<A, T>>()
        {
            pipeline.add_input(id, Box::new(input));
            Ok(BindingId(action, id))
        } else {
            Err(InputError::IncompatibleValue)
        }
    }

    pub fn configure<T, F>(&mut self, action: A, fold: F) -> Result<(), InputError>
    where
        T: IntoActionValue,
        F: InputValueFold<T>,
    {
        let pipeline = match self.bindings.get_mut(&action) {
            Some(pipeline) => pipeline,
            None => return Err(InputError::ActionNotBound),
        };

        if let Some(pipeline) = pipeline.as_any_mut().downcast_mut::<CollectingPipeline<A, T>>() {
            pipeline.configure(Box::new(fold));
            Ok(())
        } else {
            Err(InputError::IncompatibleValue)
        }
    }

    /// Bind a new pipeline allowing chaining
    pub fn with_binding<T, I>(mut self, action: A, input: I) -> Result<Self, InputError>
    where
        I: TypedInputProcessor<T>,
        T: IntoActionValue,
    {
        self.bind(action, input)?;
        Ok(self)
    }

    pub fn with_configure<T, F>(mut self, action: A, fold: F) -> Result<Self, InputError>
    where
        T: IntoActionValue,
        F: InputValueFold<T>,
    {
        self.configure(action, fold)?;
        Ok(self)
    }

    /// Add a new pipeline to an action allowing chaining
    pub fn add_binding<T, I>(&mut self, action: A, input: I) -> Result<&mut Self, InputError>
    where
        I: TypedInputProcessor<T>,
        T: IntoActionValue,
    {
        self.bind(action, input)?;
        Ok(self)
    }

    /// Unbind a pipeline by its ID.
    pub fn unbind(&mut self, id: &BindingId<A>) {
        if let Some(pipeline) = self.bindings.get_mut(&id.0) {
            pipeline.remove_input(id.1);
        }
    }

    /// Unbind all pipelines for a specific action.
    pub fn unbind_all(&mut self, action: &A) {
        self.bindings.remove(action);
    }

    pub fn get_pipeline(&self, action: &A) -> Option<&dyn InputPipeline<A>> {
        self.bindings.get(action).map(|pipeline| &**pipeline)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn integrate(&mut self, input_source: &InputDrivers) {
        for pipeline in self.bindings.values_mut() {
            pipeline.integrate(input_source);
        }
    }

    pub fn process(&mut self, time_s: f32) {
        for pipeline in self.bindings.values_mut() {
            pipeline.pull_pipeline(time_s);
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
pub fn update_action_state<A>(mut input_query: Query<(&mut InputMap<A>, &mut ActionState<A>)>)
where
    A: ActionLike,
{
    for (mut input_map, mut action_state) in input_query.iter_mut() {
        if !input_map.enabled {
            action_state.clear();
            return;
        }

        action_state.start_update();

        for (action, pipeline) in input_map.bindings.iter_mut() {
            pipeline.store_state(&mut action_state, action);
        }

        action_state.finish_update();
    }
}
