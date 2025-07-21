use crate::input_manager::InputSources;
use bevy::{math::Vec2, time::Time};
use std::any::Any;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputKind {
    Button,
    Axis,
    DualAxis,
    None,
}

pub trait UserInput: Any + Send + Sync + 'static {
    fn name(&self) -> Option<&str>;
    fn find(&self, name: &str) -> Option<&dyn UserInput>;
    fn integrate(&mut self, input: &InputSources);
}

pub trait ButtonLike: UserInput {
    fn process(&mut self, time: &Time) -> Option<bool>;
}

impl UserInput for Box<dyn ButtonLike> {
    fn name(&self) -> Option<&str> {
        self.as_ref().name()
    }

    fn find(&self, name: &str) -> Option<&dyn UserInput> {
        self.as_ref().find(name)
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
    fn name(&self) -> Option<&str> {
        self.as_ref().name()
    }

    fn find(&self, name: &str) -> Option<&dyn UserInput> {
        self.as_ref().find(name)
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
    fn name(&self) -> Option<&str> {
        self.as_ref().name()
    }

    fn find(&self, name: &str) -> Option<&dyn UserInput> {
        self.as_ref().find(name)
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
    fn find_button_component<T: ButtonLike>(&self, name: &str) -> Option<&T>
    where
        T: Sized,
    {
        self.find(name).and_then(|input| {
            let s = input as &dyn Any;
            s.downcast_ref::<T>()
        })
    }

    fn find_axis_component<T: AxisLike>(&self, name: &str) -> Option<&T> {
        self.find(name).and_then(|input| {
            let s = input as &dyn Any;
            s.downcast_ref::<T>()
        })
    }

    fn find_dual_axis_component<T: DualAxisLike>(&self, name: &str) -> Option<&T> {
        self.find(name).and_then(|input| {
            let s = input as &dyn Any;
            s.downcast_ref::<T>()
        })
    }
}

impl<T: UserInput> UserInputExt for T {}
