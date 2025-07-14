use crate::input_manager::InputKind;
use bevy::{ecs::prelude::*, math::Vec2, time::Time};
use std::{collections::HashMap, hash::Hash};

pub trait ActionLike: Clone + Eq + Hash + Send + Sync + 'static {}

impl<A> ActionLike for A where A: Clone + Eq + Hash + Send + Sync + 'static {}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonStatus {
    JustPressed,
    Pressed,
    JustReleased,
    Released,
}

#[derive(Debug, Clone)]
pub struct ButtonData {
    /// The time since the beginning of the current state
    pub start_time: f32,
    pub status: ButtonStatus,
}

impl Default for ButtonData {
    fn default() -> Self {
        Self {
            start_time: 0.0,
            status: ButtonStatus::Released,
        }
    }
}

impl ButtonData {
    pub fn just_pressed(&self) -> bool {
        matches!(self.status, ButtonStatus::JustPressed)
    }

    pub fn just_released(&self) -> bool {
        matches!(self.status, ButtonStatus::JustReleased)
    }

    pub fn is_down(&self) -> bool {
        matches!(self.status, ButtonStatus::JustPressed | ButtonStatus::Pressed)
    }

    /// Return the time since the start of this state.
    pub fn elapsed_time(&self, time: &Time) -> f32 {
        (time.elapsed().as_secs_f32() - self.start_time).max(0.0)
    }

    pub fn update(&mut self, pressed: bool, time: f32) {
        if pressed {
            match self.status {
                ButtonStatus::JustPressed => self.status = ButtonStatus::Pressed,
                ButtonStatus::Pressed => { /* keep */ }
                ButtonStatus::JustReleased | ButtonStatus::Released => {
                    self.status = ButtonStatus::JustPressed;
                    self.start_time = time;
                }
            }
        } else {
            match self.status {
                ButtonStatus::JustPressed | ButtonStatus::Pressed => {
                    self.status = ButtonStatus::JustReleased;
                    self.start_time = time;
                }
                ButtonStatus::JustReleased => self.status = ButtonStatus::Released,
                ButtonStatus::Released => { /* keep */ }
            }
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

    pub fn to_button(&self) -> ButtonData {
        match self {
            ActionData::Button(data) => data.clone(),
            _ => ButtonData::default(),
        }
    }

    pub fn as_button_mut(&mut self) -> &mut ButtonData {
        match self {
            ActionData::Button(data) => data,
            _ => panic!("Action is not a button"),
        }
    }

    pub fn to_axis(&self) -> AxisData {
        match self {
            ActionData::Axis(data) => data.clone(),
            _ => AxisData::default(),
        }
    }

    pub fn as_axis_mut(&mut self) -> &mut AxisData {
        match self {
            ActionData::Axis(data) => data,
            _ => panic!("Action is not an axis"),
        }
    }

    pub fn to_dual_axis(&self) -> DualAxisData {
        match self {
            ActionData::DualAxis(data) => data.clone(),
            _ => DualAxisData::default(),
        }
    }

    pub fn as_dual_axis_mut(&mut self) -> &mut DualAxisData {
        match self {
            ActionData::DualAxis(data) => data,
            _ => panic!("Action is not a dual axis"),
        }
    }
}

#[derive(Component, Debug, Clone)]
pub struct ActionState<A>
where
    A: ActionLike,
{
    pub version: usize,
    pub data: HashMap<A, (usize, ActionData)>,
}

impl<A> Default for ActionState<A>
where
    A: ActionLike,
{
    fn default() -> Self {
        Self {
            version: 0,
            data: HashMap::new(),
        }
    }
}

impl<A> ActionState<A>
where
    A: ActionLike,
{
    /// Clear all action data
    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn start_update(&mut self) {
        self.version += 1;
    }

    pub fn finish_update(&mut self) {
        self.data.retain(|_, data| data.0 == self.version);
    }

    pub fn kind(&self, action: &A) -> InputKind {
        match self.data.get(action) {
            Some((_, ActionData::Button(_))) => InputKind::Button,
            Some((_, ActionData::Axis(_))) => InputKind::Axis,
            Some((_, ActionData::DualAxis(_))) => InputKind::DualAxis,
            None => InputKind::None,
        }
    }

    /// Return the button state bound to the action. If not bound or not a button, a default button data.
    pub fn button(&self, action: &A) -> ButtonData {
        self.data.get(action).map(|data| data.1.to_button()).unwrap_or_default()
    }

    /// Return the bound button state. If not bound or not a button, create a new button data.
    pub fn set_button(&mut self, action: A) -> &mut ButtonData {
        let entry = self
            .data
            .entry(action)
            .or_insert_with(|| (self.version, ActionData::Button(ButtonData::default())));
        entry.0 = self.version;

        match &mut entry.1 {
            ActionData::Button(data) => data,
            data => {
                *data = ActionData::Button(ButtonData::default());
                data.as_button_mut()
            }
        }
    }

    pub fn axis(&self, action: &A) -> AxisData {
        self.data.get(action).map(|data| data.1.to_axis()).unwrap_or_default()
    }

    pub fn set_axis(&mut self, action: A) -> &mut AxisData {
        let entry = self
            .data
            .entry(action)
            .or_insert_with(|| (self.version, ActionData::Axis(AxisData::default())));
        entry.0 = self.version;

        match &mut entry.1 {
            ActionData::Axis(data) => data,
            data => {
                *data = ActionData::Axis(AxisData::default());
                data.as_axis_mut()
            }
        }
    }

    pub fn dual_axis(&self, action: &A) -> DualAxisData {
        self.data
            .get(action)
            .map(|data| data.1.to_dual_axis())
            .unwrap_or_default()
    }

    pub fn set_dual_axis(&mut self, action: A) -> &mut DualAxisData {
        let entry = self
            .data
            .entry(action)
            .or_insert_with(|| (self.version, ActionData::DualAxis(DualAxisData::default())));
        entry.0 = self.version;

        match &mut entry.1 {
            ActionData::DualAxis(data) => data,
            data => {
                *data = ActionData::DualAxis(DualAxisData::default());
                data.as_dual_axis_mut()
            }
        }
    }
}
