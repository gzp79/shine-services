use crate::input_manager::{InputSources, TypedUserInput, UserInput};
use bevy::{math::Vec2, window::Window};
use std::borrow::Cow;

/// Converts a position from screen coordinates to a normalized coordinate system, such that the smaller screen dimension
/// maps to the range [-1.0, 1.0], preserving the aspect ratio for the larger dimension.
///
/// Input: Expects a position in screen space, where (0, 0) is the top-left corner and the Y axis increases downward.
/// Output: Produces a normalized position where (0, 0) is at the center of the screen, the Y axis increases upward.
pub struct NormalizedPosition<I>
where
    I: TypedUserInput<Vec2>,
{
    name: Option<String>,
    input: I,
    screen_size: Vec2,
}

impl<I> NormalizedPosition<I>
where
    I: TypedUserInput<Vec2>,
{
    pub fn new(input: I) -> Self {
        Self {
            name: None,
            input,
            screen_size: Vec2::ZERO,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<I> UserInput for NormalizedPosition<I>
where
    I: TypedUserInput<Vec2>,
{
    fn type_name(&self) -> &'static str {
        "ViewportNormalizedPosition"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self) && self.input.visit_recursive(depth + 1, visitor)
    }

    fn integrate(&mut self, input: &InputSources) {
        self.input.integrate(input);

        if let Some(window) = input.get_resource::<Window>() {
            self.screen_size = Vec2 {
                x: window.width(),
                y: window.height(),
            };
        }
    }
}

impl<I> TypedUserInput<Vec2> for NormalizedPosition<I>
where
    I: TypedUserInput<Vec2>,
{
    fn process(&mut self, time_s: f32) -> Option<Vec2> {
        if let Some(pos) = self.input.process(time_s) {
            let w = self.screen_size.x;
            let h = self.screen_size.y;
            let s = (w.min(h) / 2.0).max(1.0);
            let mut value = Vec2::new((pos.x - w / 2.0) / s, (pos.y - h / 2.0) / s);

            // Invert the y-axis because in the input system, upward movement is positive
            value.y = -value.y;

            Some(value)
        } else {
            None
        }
    }
}

pub enum EdgeSize {
    /// Use a fixed edge size in pixels.
    Fixed(f32),
    /// Use a percentage of the screen size for the edge size.
    Percent(f32),
}

/// Converts a screen position into an edge scroll vector within the range [-1, 1] for both axes.
/// The value approaches -1 or 1 as the position nears the respective screen edge, and is 0 when away from the edge.
///
/// The input is assumed to be in screen coordinates, with the origin at the top-left corner, with positive Y pointing down.
/// The output is also in screen coordinates, with positive Y pointing down.
pub struct ScreenEdgeScroll<I>
where
    I: TypedUserInput<Vec2>,
{
    name: Option<String>,
    input: I,
    edge: EdgeSize,
    screen_size: Vec2,
}

impl<I> ScreenEdgeScroll<I>
where
    I: TypedUserInput<Vec2>,
{
    pub fn new(input: I, edge: EdgeSize) -> Self {
        Self {
            name: None,
            input,
            edge,
            screen_size: Vec2::ZERO,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<I> UserInput for ScreenEdgeScroll<I>
where
    I: TypedUserInput<Vec2>,
{
    fn type_name(&self) -> &'static str {
        "ScreenEdgeScroll"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self) && self.input.visit_recursive(depth + 1, visitor)
    }

    fn integrate(&mut self, input: &InputSources) {
        self.input.integrate(input);

        if let Some(window) = input.get_resource::<Window>() {
            self.screen_size = Vec2 {
                x: window.width(),
                y: window.height(),
            };
        }
    }
}

impl<I> TypedUserInput<Vec2> for ScreenEdgeScroll<I>
where
    I: TypedUserInput<Vec2>,
{
    fn process(&mut self, time_s: f32) -> Option<Vec2> {
        if let Some(pos) = self.input.process(time_s) {
            let w = self.screen_size.x;
            let h = self.screen_size.y;
            let (ew, eh) = match self.edge {
                EdgeSize::Fixed(size) => (size, size),
                EdgeSize::Percent(percent) => (w * percent, h * percent),
            };

            const EPS: f32 = 1e-8;
            let mut value = Vec2::ZERO;

            if ew > EPS {
                if pos.x <= ew {
                    // Left edge
                    value.x -= (1.0 - pos.x / ew).clamp(0.0, 1.0)
                }
                if pos.x >= w - ew {
                    // Right edge
                    value.x += (1.0 - (w - pos.x) / ew).clamp(0.0, 1.0);
                }
            }
            if eh > EPS {
                if pos.y <= eh {
                    // Top edge
                    value.y -= (1.0 - pos.y / eh).clamp(0.0, 1.0);
                }
                if pos.y >= self.screen_size.y - eh {
                    // Bottom edge
                    value.y += (1.0 - (h - pos.y) / eh).clamp(0.0, 1.0);
                }
            }

            Some(value)
        } else {
            None
        }
    }
}

/// Trait to extend `UserInput` with screen position processing capabilities.
pub trait ScreenPositionProcess: TypedUserInput<Vec2> {
    fn normalize_to_screen(self) -> NormalizedPosition<Self>
    where
        Self: Sized,
    {
        NormalizedPosition::new(self)
    }

    fn edge_scroll(self, edge: EdgeSize) -> ScreenEdgeScroll<Self>
    where
        Self: Sized,
    {
        ScreenEdgeScroll::new(self, edge)
    }
}

impl<T> ScreenPositionProcess for T where T: TypedUserInput<Vec2> + Sized {}
