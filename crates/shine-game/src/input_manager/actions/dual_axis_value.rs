use crate::input_manager::{InputValueFold, IntoActionValue, MaxFold};
use bevy::math::Vec2;

impl IntoActionValue for Vec2 {
    type ActionValue = DualAxisValue;

    fn default_fold() -> Box<dyn InputValueFold<Self>>
    where
        Self: Sized,
    {
        Box::new(MaxFold)
    }

    fn update_state(state: &mut Self::ActionValue, value: Option<Self>, _time_s: f32) {
        state.value = value;
    }
}

#[derive(Debug, Clone, Default)]
pub struct DualAxisValue {
    pub value: Option<Vec2>,
}
