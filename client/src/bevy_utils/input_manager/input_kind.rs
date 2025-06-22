use crate::bevy_utils::input_manager::InputSource;
use bevy::{math::Vec2, time::Time};

pub trait UserInput: Send + Sync + 'static {}

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

