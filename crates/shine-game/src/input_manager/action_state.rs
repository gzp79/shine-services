use crate::input_manager::InputKind;
use bevy::{ecs::prelude::*, math::Vec2, time::Time};
use std::{collections::HashMap, hash::Hash};

pub trait ActionLike: Clone + Eq + Hash + Send + Sync + 'static {}

impl<A> ActionLike for A where A: Clone + Eq + Hash + Send + Sync + 'static {}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonStatus {
    None,
    JustPressed,
    Pressed,
    JustReleased,
    Released,
}

#[derive(Debug, Clone)]
pub struct ButtonData {
    pub start_time: f32,
    pub status: ButtonStatus,
}

impl Default for ButtonData {
    fn default() -> Self {
        Self {
            start_time: 0.0,
            status: ButtonStatus::None,
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

    pub fn update(&mut self, pressed: Option<bool>, time: f32) {
        match pressed {
            None => {
                self.status = ButtonStatus::None;
                self.start_time = time;
            }
            Some(true) => {
                match self.status {
                    ButtonStatus::JustPressed => self.status = ButtonStatus::Pressed,
                    ButtonStatus::Pressed => { /* keep */ }
                    ButtonStatus::JustReleased | ButtonStatus::Released | ButtonStatus::None => {
                        self.status = ButtonStatus::JustPressed;
                        self.start_time = time;
                    }
                }
            }
            Some(false) => {
                match self.status {
                    ButtonStatus::JustPressed | ButtonStatus::Pressed => {
                        self.status = ButtonStatus::JustReleased;
                        self.start_time = time;
                    }
                    ButtonStatus::None | ButtonStatus::JustReleased => self.status = ButtonStatus::Released,
                    ButtonStatus::Released => { /* keep */ }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AxisData {
    pub value: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct DualAxisData {
    pub value: Option<Vec2>,
}

#[derive(Debug, Clone, Default)]
pub struct ClassificationData {
    pub value: Option<(usize, f32)>,
}

#[derive(Debug, Clone)]
enum ActionData {
    Button(ButtonData),
    Axis(AxisData),
    DualAxis(DualAxisData),
    Classification(ClassificationData),
}

impl ActionData {
    pub fn try_as_button(&self) -> Option<&ButtonData> {
        match self {
            ActionData::Button(data) => Some(data),
            _ => None,
        }
    }

    pub fn as_button_mut(&mut self) -> &mut ButtonData {
        match self {
            ActionData::Button(data) => data,
            _ => panic!("Action is not a button"),
        }
    }

    pub fn try_as_axis(&self) -> Option<&AxisData> {
        match self {
            ActionData::Axis(data) => Some(data),
            _ => None,
        }
    }

    pub fn as_axis_mut(&mut self) -> &mut AxisData {
        match self {
            ActionData::Axis(data) => data,
            _ => panic!("Action is not an axis"),
        }
    }

    pub fn try_as_dual_axis(&self) -> Option<&DualAxisData> {
        match self {
            ActionData::DualAxis(data) => Some(data),
            _ => None,
        }
    }

    pub fn as_dual_axis_mut(&mut self) -> &mut DualAxisData {
        match self {
            ActionData::DualAxis(data) => data,
            _ => panic!("Action is not a dual axis"),
        }
    }

    pub fn try_as_classification(&self) -> Option<&ClassificationData> {
        match self {
            ActionData::Classification(data) => Some(data),
            _ => None,
        }
    }

    pub fn as_classification_mut(&mut self) -> &mut ClassificationData {
        match self {
            ActionData::Classification(data) => data,
            _ => panic!("Action is not a classification"),
        }
    }
}

#[derive(Component, Debug, Clone)]
pub struct ActionState<A>
where
    A: ActionLike,
{
    version: usize,
    data: HashMap<A, (usize, ActionData)>,
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
            Some((_, ActionData::Classification(_))) => InputKind::Classification,
            None => InputKind::None,
        }
    }

    /// Return the button state bound to the action. If data is not available or not a button, None is returned.
    pub fn as_button(&self, action: &A) -> Option<&ButtonData> {
        self.data.get(action).and_then(|data| data.1.try_as_button())
    }

    /// A convenience method to get the button state. If data is not available, None is returned.
    #[inline]
    pub fn button_value(&self, action: &A) -> ButtonStatus {
        self.as_button(action).map_or(ButtonStatus::None, |data| data.status)
    }

    /// A convenience method to get the button state, returning if action was just pressed. If data is not available, false is returned.
    #[inline]
    pub fn just_pressed(&self, action: &A) -> bool {
        self.as_button(action).is_some_and(|data| data.just_pressed())
    }

    /// A convenience method to get the button state, returning if action was just released. If data is not available, false is returned.
    #[inline]
    pub fn just_released(&self, action: &A) -> bool {
        self.as_button(action).is_some_and(|data| data.just_released())
    }

    /// A convenience method to get the button state, returning if action in pressed. If data is not available, false is returned.
    #[inline]
    pub fn is_pressed(&self, action: &A) -> bool {
        self.as_button(action).is_some_and(|data| data.is_down())
    }

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

    /// Return the axis state bound to the action. If data is not available or not a axis input, None is returned.
    #[inline]
    pub fn as_axis(&self, action: &A) -> Option<&AxisData> {
        self.data.get(action).and_then(|data| data.1.try_as_axis())
    }

    /// A convenience method to get the axis value, returning None if data is not available.
    #[inline]
    pub fn try_axis_value(&self, action: &A) -> Option<f32> {
        self.as_axis(action).and_then(|data| data.value)
    }

    /// A convenience method to get the axis value, returning 0.0 if data is not available.
    #[inline]
    pub fn axis_value(&self, action: &A) -> f32 {
        self.try_axis_value(action).unwrap_or(0.0)
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

    /// Return the dual-axis state bound to the action. If data is not available or not a dual-axis input, None is returned.
    #[inline]
    pub fn as_dual_axis(&self, action: &A) -> Option<&DualAxisData> {
        self.data.get(action).and_then(|data| data.1.try_as_dual_axis())
    }

    /// A convenience method to get the dual-axis value, returning None if data is not available.
    #[inline]
    pub fn try_dual_axis_value(&self, action: &A) -> Option<Vec2> {
        self.as_dual_axis(action).and_then(|data| data.value)
    }

    /// A convenience method to get the dual-axis value, returning Vec2::ZERO if data is not available.
    #[inline]
    pub fn dual_axis_value(&self, action: &A) -> Vec2 {
        self.try_dual_axis_value(action).unwrap_or(Vec2::ZERO)
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

    /// Return the classification state bound to the action. If data is not available or not a classification input, None is returned.
    #[inline]
    pub fn as_classification(&self, action: &A) -> Option<&ClassificationData> {
        self.data.get(action).and_then(|data| data.1.try_as_classification())
    }

    /// A convenience method to get the classification value, returning None if data is not available.
    #[inline]
    pub fn try_classification_value(&self, action: &A) -> Option<(usize, f32)> {
        self.as_classification(action).and_then(|data| data.value)
    }

    pub fn set_classification(&mut self, action: A) -> &mut ClassificationData {
        let entry = self
            .data
            .entry(action)
            .or_insert_with(|| (self.version, ActionData::Classification(ClassificationData::default())));
        entry.0 = self.version;

        match &mut entry.1 {
            ActionData::Classification(data) => data,
            data => {
                *data = ActionData::Classification(ClassificationData::default());
                data.as_classification_mut()
            }
        }
    }
}
