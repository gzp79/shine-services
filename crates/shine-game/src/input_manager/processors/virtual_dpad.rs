use crate::input_manager::{
    GamepadButtonInput, InputDrivers, InputProcessor, KeyboardInput, RadialInputProcess, TypedInputProcessor,
};
use bevy::{
    ecs::entity::Entity,
    input::{gamepad::GamepadButton, keyboard::KeyCode},
    math::Vec2,
};
use std::borrow::Cow;

/// A virtual dpad that converts 4 buttons into a dual axis.
pub struct VirtualDPad<U, D, L, R>
where
    U: TypedInputProcessor<bool>,
    D: TypedInputProcessor<bool>,
    L: TypedInputProcessor<bool>,
    R: TypedInputProcessor<bool>,
{
    name: Option<String>,
    up: U,
    down: D,
    left: L,
    right: R,
}

impl<U, D, L, R> VirtualDPad<U, D, L, R>
where
    U: TypedInputProcessor<bool>,
    D: TypedInputProcessor<bool>,
    L: TypedInputProcessor<bool>,
    R: TypedInputProcessor<bool>,
{
    pub fn new(up: U, down: D, left: L, right: R) -> Self {
        Self {
            name: None,
            up,
            down,
            left,
            right,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<U, D, L, R> InputProcessor for VirtualDPad<U, D, L, R>
where
    U: TypedInputProcessor<bool>,
    D: TypedInputProcessor<bool>,
    L: TypedInputProcessor<bool>,
    R: TypedInputProcessor<bool>,
{
    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(
        &'a self,
        depth: usize,
        visitor: &mut dyn FnMut(usize, &'a dyn InputProcessor) -> bool,
    ) -> bool {
        visitor(depth, self)
            && self.up.visit_recursive(depth + 1, visitor)
            && self.down.visit_recursive(depth + 1, visitor)
            && self.left.visit_recursive(depth + 1, visitor)
            && self.right.visit_recursive(depth + 1, visitor)
    }

    fn integrate(&mut self, input: &InputDrivers) {
        self.up.integrate(input);
        self.down.integrate(input);
        self.left.integrate(input);
        self.right.integrate(input);
    }
}

impl<U, D, L, R> TypedInputProcessor<Vec2> for VirtualDPad<U, D, L, R>
where
    U: TypedInputProcessor<bool>,
    D: TypedInputProcessor<bool>,
    L: TypedInputProcessor<bool>,
    R: TypedInputProcessor<bool>,
{
    fn process(&mut self, time_s: f32) -> Option<Vec2> {
        let mut value = Vec2::ZERO;
        if self.up.process(time_s).unwrap_or(false) {
            value.y += 1.0;
        }
        if self.down.process(time_s).unwrap_or(false) {
            value.y -= 1.0;
        }
        if self.left.process(time_s).unwrap_or(false) {
            value.x -= 1.0;
        }
        if self.right.process(time_s).unwrap_or(false) {
            value.x += 1.0;
        }
        Some(value)
    }
}

impl VirtualDPad<KeyboardInput, KeyboardInput, KeyboardInput, KeyboardInput> {
    pub fn from_keys(up: KeyCode, down: KeyCode, left: KeyCode, right: KeyCode) -> impl TypedInputProcessor<Vec2> {
        Self::new(
            KeyboardInput::new(up),
            KeyboardInput::new(down),
            KeyboardInput::new(left),
            KeyboardInput::new(right),
        )
        .with_bounds(1.0)
    }

    pub fn wasd() -> impl TypedInputProcessor<Vec2> {
        Self::from_keys(KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD)
    }

    pub fn ijkl() -> impl TypedInputProcessor<Vec2> {
        Self::from_keys(KeyCode::KeyI, KeyCode::KeyK, KeyCode::KeyJ, KeyCode::KeyL)
    }
}

impl VirtualDPad<GamepadButtonInput, GamepadButtonInput, GamepadButtonInput, GamepadButtonInput> {
    pub fn gamepad_dpad(gamepad_entity: Entity) -> impl TypedInputProcessor<Vec2> {
        Self::new(
            GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadUp),
            GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadDown),
            GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadLeft),
            GamepadButtonInput::new(gamepad_entity, GamepadButton::DPadRight),
        )
        .with_bounds(1.0)
    }
}
