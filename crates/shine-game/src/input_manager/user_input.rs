use crate::input_manager::InputSources;
use std::{any::Any, borrow::Cow, fmt};

pub trait UserInput: Any + Send + Sync + 'static {
    fn name(&self) -> Cow<'_, str>;
    fn type_name(&self) -> &'static str;
    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool;
    fn integrate(&mut self, input: &InputSources);
}
pub trait TypedUserInput<T>: UserInput
where
    T: Send + Sync + 'static,
{
    fn process(&mut self, time_s: f32) -> Option<T>;
}

impl UserInput for Box<dyn UserInput> {
    fn name(&self) -> Cow<'_, str> {
        self.as_ref().name()
    }

    fn type_name(&self) -> &'static str {
        self.as_ref().type_name()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        self.as_ref().visit_recursive(depth, visitor)
    }

    fn integrate(&mut self, input: &InputSources) {
        self.as_mut().integrate(input);
    }
}

impl<T> UserInput for Box<dyn TypedUserInput<T>>
where
    T: Send + Sync + 'static,
{
    fn name(&self) -> Cow<'_, str> {
        self.as_ref().name()
    }

    fn type_name(&self) -> &'static str {
        self.as_ref().type_name()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        self.as_ref().visit_recursive(depth, visitor)
    }

    fn integrate(&mut self, input: &InputSources) {
        self.as_mut().integrate(input);
    }
}

impl<T> TypedUserInput<T> for Box<dyn TypedUserInput<T>>
where
    T: Send + Sync + 'static,
{
    fn process(&mut self, time_s: f32) -> Option<T> {
        self.as_mut().process(time_s)
    }
}

pub trait UserInputExt: UserInput {
    fn boxed(self) -> Box<dyn UserInput>
    where
        Self: Sized,
    {
        Box::new(self)
    }

    fn traverse<'a>(&'a self, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) {
        self.visit_recursive(0, visitor);
    }

    fn find_by_name<'a>(&'a self, name: &str) -> Option<&'a dyn UserInput> {
        let mut result = None;

        self.traverse(&mut |_, node| {
            if result.is_none() && node.name() == name {
                result = Some(node);
            }
            result.is_none()
        });

        result
    }

    fn find_by_name_as<T>(&self, name: &str) -> Option<&T>
    where
        T: UserInput,
    {
        self.find_by_name(name).and_then(|input| {
            let s = input as &dyn Any;
            s.downcast_ref::<T>()
        })
    }

    fn dump_pipeline<W>(&self, writer: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        let mut error = None;

        self.traverse(&mut |depth, node| {
            if let Err(e) = writeln!(
                writer,
                "{}{} ({})",
                " ".repeat(depth * 2),
                node.type_name(),
                node.name()
            ) {
                error = Some(e);
            }
            error.is_none()
        });

        if let Some(e) = error {
            Err(e)
        } else {
            Ok(())
        }
    }
}

impl<T: UserInput + ?Sized> UserInputExt for T {}
