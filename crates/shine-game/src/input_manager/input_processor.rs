use crate::input_manager::InputDrivers;
use shine_core::utils::TypeErase;
use std::borrow::Cow;

pub trait InputProcessor: TypeErase + 'static {
    fn name(&self) -> Cow<'_, str>;
    fn visit_recursive<'a>(
        &'a self,
        depth: usize,
        visitor: &mut dyn FnMut(usize, &'a dyn InputProcessor) -> bool,
    ) -> bool;
    fn integrate(&mut self, input: &InputDrivers);
}

pub trait TypedInputProcessor<T>: InputProcessor
where
    T: Send + Sync + 'static,
{
    fn process(&mut self, time_s: f32) -> Option<T>;
}

impl InputProcessor for Box<dyn InputProcessor> {
    fn name(&self) -> Cow<'_, str> {
        self.as_ref().name()
    }

    fn visit_recursive<'a>(
        &'a self,
        depth: usize,
        visitor: &mut dyn FnMut(usize, &'a dyn InputProcessor) -> bool,
    ) -> bool {
        self.as_ref().visit_recursive(depth, visitor)
    }

    fn integrate(&mut self, input: &InputDrivers) {
        self.as_mut().integrate(input);
    }
}

impl<T> InputProcessor for Box<dyn TypedInputProcessor<T>>
where
    T: Send + Sync + 'static,
{
    fn name(&self) -> Cow<'_, str> {
        self.as_ref().name()
    }

    fn visit_recursive<'a>(
        &'a self,
        depth: usize,
        visitor: &mut dyn FnMut(usize, &'a dyn InputProcessor) -> bool,
    ) -> bool {
        self.as_ref().visit_recursive(depth, visitor)
    }

    fn integrate(&mut self, input: &InputDrivers) {
        self.as_mut().integrate(input);
    }
}

impl<T> TypedInputProcessor<T> for Box<dyn TypedInputProcessor<T>>
where
    T: Send + Sync + 'static,
{
    fn process(&mut self, time_s: f32) -> Option<T> {
        self.as_mut().process(time_s)
    }
}
