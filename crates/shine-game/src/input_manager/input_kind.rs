use crate::input_manager::InputSources;
use bevy::math::Vec2;

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
    fn is_down(&self) -> bool;
}

pub trait AxisLike: UserInput {
    fn value(&self) -> f32;
}

pub trait DualAxisLike: UserInput {
    fn value_pair(&self) -> Vec2;
}
