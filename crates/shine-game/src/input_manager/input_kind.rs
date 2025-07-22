use crate::input_manager::InputSources;
use bevy::{math::Vec2, time::Time};
use std::{any::Any, borrow::Cow, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputKind {
    Button,
    Axis,
    DualAxis,
    None,
}

pub trait UserInput: Any + Send + Sync + 'static {
    fn name(&self) -> Cow<'_, str>;
    fn type_name(&self) -> &'static str;
    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool;
    fn integrate(&mut self, input: &InputSources);
}

pub trait ButtonLike: UserInput {
    fn process(&mut self, time: &Time) -> Option<bool>;
}

impl UserInput for Box<dyn ButtonLike> {
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

impl ButtonLike for Box<dyn ButtonLike> {
    fn process(&mut self, time: &Time) -> Option<bool> {
        self.as_mut().process(time)
    }
}

pub trait AxisLike: UserInput {
    fn process(&mut self, time: &Time) -> Option<f32>;
}

impl UserInput for Box<dyn AxisLike> {
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

impl AxisLike for Box<dyn AxisLike> {
    fn process(&mut self, time: &Time) -> Option<f32> {
        self.as_mut().process(time)
    }
}

pub trait DualAxisLike: UserInput {
    fn process(&mut self, time: &Time) -> Option<Vec2>;
}

impl UserInput for Box<dyn DualAxisLike> {
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

impl DualAxisLike for Box<dyn DualAxisLike> {
    fn process(&mut self, time: &Time) -> Option<Vec2> {
        self.as_mut().process(time)
    }
}

pub trait UserInputExt: UserInput {
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

    fn find_button_component<T>(&self, name: &str) -> Option<&T>
    where
        T: ButtonLike,
    {
        self.find_by_name(name).and_then(|input| {
            let s = input as &dyn Any;
            s.downcast_ref::<T>()
        })
    }

    fn find_axis_component<T>(&self, name: &str) -> Option<&T>
    where
        T: AxisLike,
    {
        self.find_by_name(name).and_then(|input| {
            let s = input as &dyn Any;
            s.downcast_ref::<T>()
        })
    }

    fn find_dual_axis_component<T>(&self, name: &str) -> Option<&T>
    where
        T: DualAxisLike,
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
            if let Err(e) = write!(
                writer,
                "{}{} ({})\n",
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
