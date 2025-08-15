use crate::input_manager::{InputSources, TypedUserInput, UserInput};
use bevy::math::Vec2;
use std::borrow::Cow;

/// A dual axis that returns value only when the button is pressed.
pub struct DualAxisChord<B, D>
where
    B: TypedUserInput<bool>,
    D: TypedUserInput<Vec2>,
{
    name: Option<String>,
    button: B,
    dual_axis: D,
}

impl<B, D> DualAxisChord<B, D>
where
    B: TypedUserInput<bool>,
    D: TypedUserInput<Vec2>,
{
    pub fn new(button: B, dual_axis: D) -> Self {
        Self { name: None, button, dual_axis }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<B, D> UserInput for DualAxisChord<B, D>
where
    B: TypedUserInput<bool>,
    D: TypedUserInput<Vec2>,
{
    fn type_name(&self) -> &'static str {
        "DualAxisChord"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
            && self.button.visit_recursive(depth + 1, visitor)
            && self.dual_axis.visit_recursive(depth + 1, visitor)
    }

    fn integrate(&mut self, input: &InputSources) {
        self.button.integrate(input);
        self.dual_axis.integrate(input);
    }
}

impl<B, D> TypedUserInput<Vec2> for DualAxisChord<B, D>
where
    B: TypedUserInput<bool>,
    D: TypedUserInput<Vec2>,
{
    fn process(&mut self, time_s: f32) -> Option<Vec2> {
        let button = self.button.process(time_s).unwrap_or(false);
        let value = self.dual_axis.process(time_s);

        if button {
            value
        } else {
            None
        }
    }
}
