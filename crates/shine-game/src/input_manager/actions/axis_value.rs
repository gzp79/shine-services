use crate::input_manager::{InputValueFold, IntoActionValue, MaxFold};

impl IntoActionValue for f32 {
    type ActionValue = AxisValue;

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

#[derive(Debug, Default, Clone)]
pub struct AxisValue {
    pub value: Option<f32>,
}
