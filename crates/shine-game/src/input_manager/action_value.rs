use crate::input_manager::{AndFold, InputValueFold, IntoActionValue, MaxFold};
use bevy::{math::Vec2, time::Time};
use std::any::Any;

/// Marker trait for types that represent the state or value of an input action.
///
/// Implement this trait for any type that you want to use as an action value
/// in the input system (e.g., button, axis, or dual-axis values).
pub trait ActionValue: Sync + Send + 'static {}

/// Blank trait for type erased action values.
pub trait AnyActionValue: ActionValue + Any {
    /// Returns `self` as `&dyn Any`
    fn as_any(&self) -> &dyn std::any::Any;

    /// Returns `self` as `&mut dyn Any`
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T> AnyActionValue for T
where
    T: ActionValue + Any,
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonStatus {
    None,
    JustPressed,
    Pressed,
    JustReleased,
    Released,
}

#[derive(Debug, Clone)]
pub struct ButtonValue {
    pub input: Option<bool>,

    pub start_time: f32,
    pub status: ButtonStatus,
}

impl Default for ButtonValue {
    fn default() -> Self {
        Self {
            input: None,

            start_time: 0.0,
            status: ButtonStatus::None,
        }
    }
}

impl ButtonValue {
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
        (time.elapsed_secs() - self.start_time).max(0.0)
    }

    pub fn update(&mut self, pressed: Option<bool>, time_s: f32) {
        match pressed {
            None => {
                self.status = ButtonStatus::None;
                self.start_time = time_s;
            }
            Some(true) => {
                match self.status {
                    ButtonStatus::JustPressed => self.status = ButtonStatus::Pressed,
                    ButtonStatus::Pressed => { /* keep */ }
                    ButtonStatus::JustReleased | ButtonStatus::Released | ButtonStatus::None => {
                        self.status = ButtonStatus::JustPressed;
                        self.start_time = time_s;
                    }
                }
            }
            Some(false) => {
                match self.status {
                    ButtonStatus::JustPressed | ButtonStatus::Pressed => {
                        self.status = ButtonStatus::JustReleased;
                        self.start_time = time_s;
                    }
                    ButtonStatus::None | ButtonStatus::JustReleased => self.status = ButtonStatus::Released,
                    ButtonStatus::Released => { /* keep */ }
                }
            }
        }
    }
}

impl ActionValue for ButtonValue {}

impl IntoActionValue for bool {
    type State = ButtonValue;

    fn default_fold() -> Box<dyn InputValueFold<Self>>
    where
        Self: Sized,
    {
        Box::new(AndFold)
    }

    fn update_state(state: &mut Self::State, value: Option<Self>, time_s: f32) {
        state.update(value, time_s);
    }
}

#[derive(Debug, Default, Clone)]
pub struct AxisValue {
    pub value: Option<f32>,
}

impl ActionValue for AxisValue {}

impl IntoActionValue for f32 {
    type State = AxisValue;

    fn default_fold() -> Box<dyn InputValueFold<Self>>
    where
        Self: Sized,
    {
        Box::new(MaxFold)
    }

    fn update_state(state: &mut Self::State, value: Option<Self>, _time_s: f32) {
        state.value = value;
    }
}

#[derive(Debug, Clone, Default)]
pub struct DualAxisValue {
    pub value: Option<Vec2>,
}

impl ActionValue for DualAxisValue {}

impl IntoActionValue for Vec2 {
    type State = DualAxisValue;

    fn default_fold() -> Box<dyn InputValueFold<Self>>
    where
        Self: Sized,
    {
        Box::new(MaxFold)
    }

    fn update_state(state: &mut Self::State, value: Option<Self>, _time_s: f32) {
        state.value = value;
    }
}
