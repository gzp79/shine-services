use bevy::{ecs::prelude::*, math::Vec2};
use std::{collections::HashMap, hash::Hash};

pub trait ActionLike: Eq + Hash + Send + Sync + 'static {}

#[derive(Debug, Clone)]
pub struct ButtonData {
    /// The time since the beginning of the current state
    pub start_time: f32,
    pub pressed: bool,
}

impl Default for ButtonData {
    fn default() -> Self {
        Self {
            start_time: 0.0,
            pressed: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AxisData {
    pub value: f32,
}

impl Default for AxisData {
    fn default() -> Self {
        Self { value: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct DualAxisData {
    pub value: Vec2,
}

impl Default for DualAxisData {
    fn default() -> Self {
        Self { value: Vec2::ZERO }
    }
}

#[derive(Debug, Clone)]
pub enum ActionData {
    Button(ButtonData),
    Axis(AxisData),
    DualAxis(DualAxisData),
}

impl ActionData {
    pub fn clear(&mut self) {
        match self {
            ActionData::Button(data) => *data = ButtonData::default(),
            ActionData::Axis(data) => *data = AxisData::default(),
            ActionData::DualAxis(data) => *data = DualAxisData::default(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct ActionState<A>
where
    A: ActionLike,
{
    pub data: HashMap<A, ActionData>,
}

impl<A> Default for ActionState<A>
where
    A: ActionLike,
{
    fn default() -> Self {
        Self { data: HashMap::new() }
    }
}

impl<A> ActionState<A>
where
    A: ActionLike,
{
    pub fn clear(&mut self) {
        for data in self.data.values_mut() {
            data.clear();
        }
    }

    /// Return the button state bound to the action
    /// # Panics
    /// Panics if the action is not bound to a button
    pub fn button_data(&self, action: &A) -> &ButtonData {
        match self.data.get(action) {
            Some(ActionData::Button(data)) => data,
            _ => panic!("Action is not bound to a button"),
        }
    }

    /// Return the button state bound to the action
    /// # Panics
    /// Panics if the action is not bound to a button
    pub fn button_data_mut(&mut self, action: &A) -> &mut ButtonData {
        match self.data.get_mut(action) {
            Some(ActionData::Button(data)) => data,
            _ => panic!("Action is not bound to a button"),
        }
    }

    /// Return the axis state bound to the action
    /// # Panics
    /// Panics if the action is not bound to an axis
    pub fn axis_data(&self, action: &A) -> &AxisData {
        match self.data.get(action) {
            Some(ActionData::Axis(data)) => data,
            _ => panic!("Action is not bound to an axis"),
        }
    }

    /// Return the axis state bound to the action
    /// # Panics
    /// Panics if the action is not bound to an axis
    pub fn axis_data_mut(&mut self, action: &A) -> &mut AxisData {
        match self.data.get_mut(action) {
            Some(ActionData::Axis(data)) => data,
            _ => panic!("Action is not bound to an axis"),
        }
    }

    /// Return the dual axis state bound to the action
    /// # Panics
    /// Panics if the action is not bound to a dual axis
    pub fn dual_axis_data(&self, action: &A) -> &DualAxisData {
        match self.data.get(action) {
            Some(ActionData::DualAxis(data)) => data,
            _ => panic!("Action is not bound to a dual axis"),
        }
    }

    /// Return the dual axis state bound to the action
    /// # Panics
    /// Panics if the action is not bound to a dual axis
    pub fn dual_axis_data_mut(&mut self, action: &A) -> &mut DualAxisData {
        match self.data.get_mut(action) {
            Some(ActionData::DualAxis(data)) => data,
            _ => panic!("Action is not bound to a dual axis"),
        }
    }
}
