use crate::input_manager::{DualAxisLike, InputSources, UserInput};
use bevy::{math::Vec2, time::Time, window::Window};

/// Return normalized screen position.
/// The value for the smaller screen dimension is in the range [-1.0, 1.0],
/// the larger dimension is kept proportional to keep the aspect ratio.
pub struct ScreenNormalizedPosition<I>
where
    I: DualAxisLike,
{
    input: I,
    screen_size: Vec2,
}

impl<I> ScreenNormalizedPosition<I>
where
    I: DualAxisLike,
{
    pub fn new(input: I) -> Self {
        Self { input, screen_size: Vec2::ZERO }
    }
}

impl<I> UserInput for ScreenNormalizedPosition<I>
where
    I: DualAxisLike,
{
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

impl<I> DualAxisLike for ScreenNormalizedPosition<I>
where
    I: DualAxisLike,
{
    fn process(&mut self, time: &Time) -> Vec2 {
        let pos = self.input.process(time);

        // Vec2::MAX is treated like a "None"
        if pos == Vec2::MAX {
            return pos;
        }

        let w = self.screen_size.x;
        let h = self.screen_size.y;
        let s = (w.min(h) / 2.0).max(1.0);
        let mut value = Vec2::new((pos.x - w / 2.0) / s, (pos.y - h / 2.0) / s);

        // Invert the y-axis because in the input system, upward movement is positive
        value.y = -value.y;

        value
    }
}

pub enum EdgeSize {
    /// Use a fixed edge size in pixels.
    Fixed(f32),
    /// Use a percentage of the screen size for the edge size.
    Percent(f32),
}

/// Interpret the position as an edge scroll dual axis value in the [-1,1]^2 range.
pub struct ScreenEdgeScroll<I>
where
    I: DualAxisLike,
{
    input: I,
    edge: EdgeSize,
    screen_size: Vec2,
}

impl<I> ScreenEdgeScroll<I>
where
    I: DualAxisLike,
{
    pub fn new(input: I, edge: EdgeSize) -> Self {
        Self {
            input,
            edge,
            screen_size: Vec2::ZERO,
        }
    }
}

impl<I> UserInput for ScreenEdgeScroll<I>
where
    I: DualAxisLike,
{
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

impl<I> DualAxisLike for ScreenEdgeScroll<I>
where
    I: DualAxisLike,
{
    fn process(&mut self, time: &Time) -> Vec2 {
        let pos = self.input.process(time);

        // Vec2::MAX is treated as a no-position
        if pos == Vec2::MAX {
            return Vec2::ZERO;
        }

        let w = self.screen_size.x;
        let h = self.screen_size.y;
        let (ew, eh) = match self.edge {
            EdgeSize::Fixed(size) => (size, size),
            EdgeSize::Percent(percent) => (w * percent, h * percent),
        };

        const EPS: f32 = 1e-4;
        let mut value = Vec2::ZERO;

        if ew > EPS {
            if pos.x <= ew {
                // Left edge
                value.x += (1.0 - pos.x / ew).clamp(0.0, 1.0)
            }
            if pos.x >= w - ew {
                // Right edge
                value.x -= (1.0 - (w - pos.x) / ew).clamp(0.0, 1.0);
            }
        }
        if eh > EPS {
            if pos.y <= eh {
                // Top edge
                value.y += (1.0 - pos.y / eh).clamp(0.0, 1.0);
            }
            if pos.y >= self.screen_size.y - eh {
                // Bottom edge
                value.y -= (1.0 - (h - pos.y) / eh).clamp(0.0, 1.0);
            }
        }

        value
    }
}

/// Helper to add circle bounds processing to an [`DualAxisLike`] input.
pub trait ScreenPositionProcessor: DualAxisLike {
    fn normalize_to_screen(self) -> ScreenNormalizedPosition<Self>
    where
        Self: Sized;

    fn edge_scroll(self, edge: EdgeSize) -> ScreenEdgeScroll<Self>
    where
        Self: Sized;
}

impl<T: DualAxisLike> ScreenPositionProcessor for T {
    fn normalize_to_screen(self) -> ScreenNormalizedPosition<Self>
    where
        Self: Sized,
    {
        ScreenNormalizedPosition::new(self)
    }

    fn edge_scroll(self, edge: EdgeSize) -> ScreenEdgeScroll<Self>
    where
        Self: Sized,
    {
        ScreenEdgeScroll::new(self, edge)
    }
}
