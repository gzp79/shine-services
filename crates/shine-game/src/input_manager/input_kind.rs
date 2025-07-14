use crate::input_manager::InputSources;
use bevy::{math::Vec2, time::Time};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputKind {
    Button,
    Axis,
    DualAxis,
    None,
}

pub trait UserInput: Send + Sync + 'static {
    fn integrate(&mut self, input: &InputSources);
}

pub trait ButtonLike: UserInput {
    fn process(&mut self, time: &Time) -> bool;
}

pub trait AxisLike: UserInput {
    fn process(&mut self, time: &Time) -> f32;
}

pub trait DualAxisLike: UserInput {
    fn process(&mut self, time: &Time) -> Vec2;
}
