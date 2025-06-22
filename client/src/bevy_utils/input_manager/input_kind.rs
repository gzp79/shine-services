use crate::bevy_utils::input_manager::InputSources;
use bevy::math::Vec2;

pub trait UserInput: Send + Sync + 'static {
    fn integrate(&mut self, input: &InputSources);
}

pub trait ButtonLike: UserInput {
    fn pressed(&self) -> bool;
    fn released(&self) -> bool;
    fn is_down(&self) -> bool;
}

pub trait AxisLike: UserInput {
    fn value(&self) -> f32;
}

pub trait DualAxisLike: UserInput {
    fn value_pair(&self) -> Vec2;
}
