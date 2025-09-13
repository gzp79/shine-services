use crate::input_manager::{
    ActionLike, ActionState, InputDrivers, InputProcessor, InputProcessorExt, InputValueFold, IntoActionValue,
    TypedInputProcessor,
};
use shine_core::utils::{TypeErase, TypeEraseExt};
use std::{fmt, marker::PhantomData};

pub trait InputPipeline<A>: TypeErase + 'static
where
    A: ActionLike,
{
    fn remove_input(&mut self, id: usize);

    fn integrate(&mut self, input_source: &InputDrivers);
    fn pull_pipeline(&mut self, time_s: f32);
    fn store_state(&mut self, action_state: &mut ActionState<A>, action: &A);

    /// Traverses all input processors in the pipeline.
    /// The provided visitor function is invoked for each processor with:
    /// - the binding ID,
    /// - the depth of the processor within the pipeline for that binding,
    /// - a reference to the processor.
    fn traverse(&self, visitor: &mut dyn FnMut(usize, usize, &dyn InputProcessor) -> bool);
}

pub trait TypedInputPipeline<A, T>: InputPipeline<A>
where
    A: ActionLike,
    T: IntoActionValue,
{
    fn add_input(&mut self, id: usize, input: Box<dyn TypedInputProcessor<T>>);
    fn get_input(&self, id: usize) -> Option<&dyn TypedInputProcessor<T>>;

    fn configure(&mut self, fold: Box<dyn InputValueFold<T>>);
}

/// Store the result of an input processing pipeline and converts it into an `ActionState`.
pub struct CollectingPipeline<A, T>
where
    A: ActionLike,
    T: IntoActionValue,
{
    inputs: Vec<(usize, Box<dyn TypedInputProcessor<T>>, Option<T>)>,
    time_s: f32,
    fold: Box<dyn InputValueFold<T>>,
    _ph: PhantomData<A>,
}

impl<A, T> Default for CollectingPipeline<A, T>
where
    A: ActionLike,
    T: IntoActionValue,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A, T> CollectingPipeline<A, T>
where
    A: ActionLike,
    T: IntoActionValue,
{
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            time_s: 0.0,
            fold: T::default_fold(),
            _ph: PhantomData,
        }
    }
}

impl<A, T> InputPipeline<A> for CollectingPipeline<A, T>
where
    A: ActionLike,
    T: IntoActionValue,
{
    fn remove_input(&mut self, id: usize) {
        self.inputs.retain(|(pid, _, _)| *pid != id);
    }

    fn integrate(&mut self, input_source: &InputDrivers) {
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

    fn store_state(&mut self, action_state: &mut ActionState<A>, action: &A) {
        let mut cumulated_value = None;

        for (_, _, value) in self.inputs.iter_mut() {
            cumulated_value = self.fold.fold(cumulated_value.take(), value.take());
        }

        let state = action_state.set_as::<T::ActionValue>(action.clone());
        T::update_state(state, cumulated_value, self.time_s);
    }

    fn traverse(&self, visitor: &mut dyn FnMut(usize, usize, &dyn InputProcessor) -> bool) {
        for (id, input, _) in &self.inputs {
            input.traverse(&mut |depth, node| visitor(*id, depth, node));
        }
    }
}

impl<A, T> TypedInputPipeline<A, T> for CollectingPipeline<A, T>
where
    A: ActionLike,
    T: IntoActionValue,
{
    fn add_input(&mut self, id: usize, input: Box<dyn TypedInputProcessor<T>>) {
        assert!(
            !self.inputs.iter().any(|(pid, _, _)| *pid == id),
            "Input with this ID already exists"
        );

        self.inputs.push((id, input, None));
    }

    fn get_input(&self, id: usize) -> Option<&dyn TypedInputProcessor<T>> {
        self.inputs
            .iter()
            .find(|(pid, _, _)| *pid == id)
            .map(|(_, input, _)| input.as_ref())
    }

    fn configure(&mut self, fold: Box<dyn InputValueFold<T>>) {
        self.fold = fold;
    }
}

pub trait InputPipelineExt<A>: InputPipeline<A>
where
    A: ActionLike,
{
    fn dump<W>(&self, writer: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        let mut error = None;

        self.traverse(&mut |id, depth, node| {
            if depth == 0 {
                if let Err(e) = write!(writer, "[{id:2}] ") {
                    error = Some(e);
                    return false;
                }
            } else if let Err(e) = write!(writer, "    ") {
                error = Some(e);
                return false;
            }

            if let Err(e) = writeln!(
                writer,
                "{}{} ({})",
                " ".repeat(depth * 2),
                node.simple_type_name(),
                node.name()
            ) {
                error = Some(e);
                return false;
            }

            true
        });

        if let Some(e) = error {
            Err(e)
        } else {
            Ok(())
        }
    }
}

impl<A, T> InputPipelineExt<A> for T
where
    A: ActionLike,
    T: ?Sized + InputPipeline<A>,
{
}
