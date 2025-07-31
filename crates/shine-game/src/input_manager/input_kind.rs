use crate::input_manager::{ActionState, IntoActionState};
use bevy::{math::Vec2, time::Time};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonStatus {
    None,
    JustPressed,
    Pressed,
    JustReleased,
    Released,
}

#[derive(Debug, Clone)]
pub struct ButtonState {
    pub input: Option<bool>,

    pub start_time: f32,
    pub status: ButtonStatus,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            input: None,

            start_time: 0.0,
            status: ButtonStatus::None,
        }
    }
}

impl ButtonState {
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

impl ActionState for ButtonState {}

impl IntoActionState for bool {
    type State = ButtonState;

    fn accumulate(state: Option<Self>, value: Option<Self>) -> Option<Self>
    where
        Self: Sized,
    {
        match (state, value) {
            (Some(prev), Some(current)) => Some(prev || current),
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    }

    fn update_state(state: &mut Self::State, value: Option<Self>, time_s: f32) {
        state.update(value, time_s);
    }
}

#[derive(Debug, Default, Clone)]
pub struct AxisState {
    pub value: Option<f32>,
}

impl ActionState for AxisState {}

impl IntoActionState for f32 {
    type State = AxisState;

    fn accumulate(prev: Option<Self>, current: Option<Self>) -> Option<Self>
    where
        Self: Sized,
    {
        match (prev, current) {
            (Some(v1), Some(v2)) => Some(v1.max(v2)),
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    }

    fn update_state(state: &mut Self::State, value: Option<Self>, _time_s: f32) {
        state.value = value;
    }
}

#[derive(Debug, Clone, Default)]
pub struct DualAxisState {
    pub value: Option<Vec2>,
}

impl ActionState for DualAxisState {}

impl IntoActionState for Vec2 {
    type State = DualAxisState;

    fn accumulate(prev: Option<Self>, current: Option<Self>) -> Option<Self>
    where
        Self: Sized,
    {
        match (prev, current) {
            (Some(v1), Some(v2)) => {
                if v1.length_squared() >= v2.length_squared() {
                    Some(v1)
                } else {
                    Some(v2)
                }
            }
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    }

    fn update_state(state: &mut Self::State, value: Option<Self>, _time_s: f32) {
        state.value = value;
    }
}
