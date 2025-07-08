use crate::input_manager::{ButtonLike, CircleBoundsProcessor, DualAxisLike, InputSources, KeyboardInput, UserInput};
use bevy::{input::keyboard::KeyCode, math::Vec2};

/// A virtual dpad that converts 4 buttons into a dual axis.
pub struct VirtualDpad<U, D, L, R>
where
    U: ButtonLike,
    D: ButtonLike,
    L: ButtonLike,
    R: ButtonLike,
{
    up: U,
    down: D,
    left: L,
    right: R,
}

impl<U, D, L, R> VirtualDpad<U, D, L, R>
where
    U: ButtonLike,
    D: ButtonLike,
    L: ButtonLike,
    R: ButtonLike,
{
    pub fn new(up: U, down: D, left: L, right: R) -> Self {
        Self { up, down, left, right }
    }
}

impl<U, D, L, R> UserInput for VirtualDpad<U, D, L, R>
where
    U: ButtonLike,
    D: ButtonLike,
    L: ButtonLike,
    R: ButtonLike,
{
    fn integrate(&mut self, input: &InputSources) {
        self.up.integrate(input);
        self.down.integrate(input);
        self.left.integrate(input);
        self.right.integrate(input);
    }
}

impl<U, D, L, R> DualAxisLike for VirtualDpad<U, D, L, R>
where
    U: ButtonLike,
    D: ButtonLike,
    L: ButtonLike,
    R: ButtonLike,
{
    fn value_pair(&self) -> Vec2 {
        let mut value = Vec2::ZERO;
        if self.up.is_down() {
            value.y += 1.0;
        }
        if self.down.is_down() {
            value.y -= 1.0;
        }
        if self.left.is_down() {
            value.x -= 1.0;
        }
        if self.right.is_down() {
            value.x += 1.0;
        }
        value
    }
}

impl VirtualDpad<KeyboardInput, KeyboardInput, KeyboardInput, KeyboardInput> {
    pub fn wasd() -> impl DualAxisLike {
        Self::new(
            KeyboardInput::new(KeyCode::KeyW),
            KeyboardInput::new(KeyCode::KeyS),
            KeyboardInput::new(KeyCode::KeyA),
            KeyboardInput::new(KeyCode::KeyD),
        )
        .with_circle_bounds(1.0)
    }

    pub fn ijkl() -> impl DualAxisLike {
        Self::new(
            KeyboardInput::new(KeyCode::KeyI),
            KeyboardInput::new(KeyCode::KeyK),
            KeyboardInput::new(KeyCode::KeyJ),
            KeyboardInput::new(KeyCode::KeyL),
        )
        .with_circle_bounds(1.0)
    }
}
