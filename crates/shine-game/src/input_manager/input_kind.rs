use crate::input_manager::IntoActionState;
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
    pub start_time: f32,
    pub status: ButtonStatus,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
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

impl IntoActionState for bool {
    type State = ButtonState;

    fn update_action_state(&self, state: &mut Self::State, time_s: f32) {
        state.update(Some(*self), time_s);
    }
}

#[derive(Debug, Clone, Default)]
pub struct AxisState {
    pub value: Option<f32>,
}

impl IntoActionState for f32 {
    type State = AxisState;

    fn update_action_state(&self, state: &mut Self::State, _time_s: f32) {
        state.value = Some(*self);
    }
}

#[derive(Debug, Clone, Default)]
pub struct DualAxisState {
    pub value: Option<Vec2>,
}

impl IntoActionState for Vec2 {
    type State = DualAxisState;

    fn update_action_state(&self, state: &mut Self::State, _time_s: f32) {
        state.value = Some(*self);
    }
}
