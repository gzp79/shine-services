use crate::input_manager::{ActionLike, AxisLike, DualAxisLike, InputMap, InputSource, InputSources, UserInput};
use bevy::{
    ecs::{
        error::BevyError,
        resource::Resource,
        system::{Query, Res, ResMut},
    },
    input::touch::Touches,
    math::Vec2,
    time::Time,
    window::Window,
};
use std::borrow::Cow;

/// Stores the positions of two touch points (fingers) during a pinch gesture,
/// including their initial, previous, and current positions. This enables
/// calculation of pan, zoom, and rotation deltas for multi-touch interactions.
#[derive(Debug, Clone)]
pub struct PinchGesturePositions {
    pub start: (Vec2, Vec2),
    pub prev: (Vec2, Vec2),
    pub current: (Vec2, Vec2),
}

impl PinchGesturePositions {
    pub fn delta_pan(&self) -> Vec2 {
        let prev = (self.prev.0 + self.prev.1) / 2.0;
        let current = (self.current.0 + self.current.1) / 2.0;
        current - prev
    }

    pub fn total_pan(&self) -> Vec2 {
        let prev = (self.start.0 + self.start.1) / 2.0;
        let current = (self.current.0 + self.current.1) / 2.0;
        current - prev
    }

    pub fn delta_zoom(&self) -> f32 {
        let prev = (self.prev.1 - self.prev.0).length();
        let current = (self.current.1 - self.current.0).length();

        // For degenerate cases, return no-zoom
        if prev < 1.0 || current < 1.0 {
            1.0
        } else {
            current / prev
        }
    }

    pub fn total_zoom(&self) -> f32 {
        let prev = (self.start.1 - self.start.0).length();
        let current = (self.current.1 - self.current.0).length();

        // For degenerate cases, return no-zoom
        if prev < 1.0 || current < 1.0 {
            1.0
        } else {
            current / prev
        }
    }

    pub fn delta_rotate(&self) -> f32 {
        let prev = self.prev.1 - self.prev.0;
        let current = self.current.1 - self.current.0;

        // For degenerate cases, return no-rotate
        if prev.length_squared() < 1.0 || current.length_squared() < 1.0 {
            0.0
        } else {
            prev.angle_to(current)
        }
    }

    pub fn total_rotate(&self) -> f32 {
        let prev = self.start.1 - self.start.0;
        let current = self.current.1 - self.current.0;

        // For degenerate cases, return no-rotate
        if prev.length_squared() < 1.0 || current.length_squared() < 1.0 {
            0.0
        } else {
            prev.angle_to(current)
        }
    }

    pub fn center(&self) -> Vec2 {
        (self.current.0 + self.current.1) / 2.0
    }
}

/// Resource that tracks the state of a two-finger touch gesture, including the IDs of the
/// active touch points and their positions. This is used to calculate
/// pan, zoom, and rotation deltas for multi-touch interactions.
#[derive(Debug, Clone, Default, Resource)]
pub struct PinchGestureState {
    pub first_id: Option<u64>,
    pub second_id: Option<u64>,
    pub positions: Option<PinchGesturePositions>,
}

impl InputSource for PinchGestureState {}

pub fn update_two_finger_touch_gesture(mut gesture: ResMut<PinchGestureState>, touches: Res<Touches>) {
    // Check if the touches are still active
    if let Some(id) = gesture.first_id {
        if touches.get_pressed(id).is_none() {
            gesture.first_id = None;
        }
    }
    if let Some(id) = gesture.second_id {
        if touches.get_pressed(id).is_none() {
            gesture.second_id = None;
        }
    }

    // Assign new ids from the touches just pressed (ignore old touches).
    for touch in touches.iter_just_pressed() {
        if gesture.first_id.is_none() {
            gesture.first_id = Some(touch.id());
        } else if gesture.second_id.is_none() && touch.id() != gesture.first_id.unwrap() {
            gesture.second_id = Some(touch.id());
            break;
        }
    }

    if let (Some(touch1), Some(touch2)) = (
        gesture.first_id.and_then(|id| touches.get_pressed(id)),
        gesture.second_id.and_then(|id| touches.get_pressed(id)),
    ) {
        gesture.positions = Some(PinchGesturePositions {
            start: (touch1.start_position(), touch2.start_position()),
            prev: (touch1.previous_position(), touch2.previous_position()),
            current: (touch1.position(), touch2.position()),
        });
    } else {
        gesture.positions = None;
    }
}

pub fn integrate_two_finger_touch_inputs<A>(
    time: Res<Time>,
    window: Query<&Window>,
    gesture: Res<PinchGestureState>,
    mut input_query: Query<&mut InputMap<A>>,
) -> Result<(), BevyError>
where
    A: ActionLike,
{
    let window = window.single()?;

    for mut input_map in input_query.iter_mut() {
        let mut input_source = InputSources::new();

        input_source.add_resource(window);
        input_source.add_resource(&*time);
        input_source.add_resource(&*gesture);

        input_map.integrate(input_source);
    }

    Ok(())
}

/// Represents the pan (translation) of a two-finger pinch gesture.
///
/// The returned [`Vec2`] is in screen coordinates (pixels), with the Y axis pointing down,
/// matching UI/screen conventions (not world coordinates).
///
/// When the gesture is not active, the value is `None`.
pub struct PinchPan {
    name: Option<String>,
    is_delta: bool,
    value: Option<Vec2>,
}

impl PinchPan {
    /// Creates a [`PinchPan`] configured to compute the pan delta since the previous frame.
    #[inline]
    pub fn delta() -> Self {
        Self {
            name: None,
            is_delta: true,
            value: None,
        }
    }

    /// Creates a [`PinchPan`] configured to compute the total pan offset since the gesture started.
    #[inline]
    pub fn total() -> Self {
        Self {
            name: None,
            is_delta: false,
            value: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl UserInput for PinchPan {
    fn type_name(&self) -> &'static str {
        "PinchPan"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

    fn integrate(&mut self, input: &InputSources) {
        if let Some(gesture) = input.get_resource::<PinchGestureState>() {
            if let Some(positions) = gesture.positions.as_ref() {
                self.value = Some(if self.is_delta {
                    positions.delta_pan()
                } else {
                    positions.total_pan()
                });
            } else {
                self.value = None;
            }
        }
    }
}

impl DualAxisLike for PinchPan {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        self.value
    }
}

/// Represents the zoom (scale) factor of a two-finger pinch gesture.
///
/// The returned `f32` is a scale factor, where 1.0 means no zoom, values greater than 1.0 mean zoom in,
/// and values less than 1.0 mean zoom out.
///
/// When the gesture is not active, the value is `None`.
pub struct PinchZoom {
    name: Option<String>,
    is_delta: bool,
    value: Option<f32>,
}

impl PinchZoom {
    /// Creates a [`PinchZoom`] configured to compute the zoom delta since the previous frame.
    #[inline]
    pub fn delta() -> Self {
        Self {
            name: None,
            is_delta: true,
            value: None,
        }
    }

    /// Creates a [`PinchZoom`] configured to compute the total zoom factor since the gesture started.
    #[inline]
    pub fn total() -> Self {
        Self {
            name: None,
            is_delta: false,
            value: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl UserInput for PinchZoom {
    fn type_name(&self) -> &'static str {
        "PinchZoom"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

    fn integrate(&mut self, input: &InputSources) {
        if let Some(gesture) = input.get_resource::<PinchGestureState>() {
            if let Some(positions) = &gesture.positions {
                self.value = Some(if self.is_delta {
                    positions.delta_zoom()
                } else {
                    positions.total_zoom()
                });
            } else {
                self.value = None;
            }
        }
    }
}

impl AxisLike for PinchZoom {
    fn process(&mut self, _time: &Time) -> Option<f32> {
        self.value
    }
}

/// Represents the rotation angle of a two-finger pinch gesture.
///
/// Returns the rotation angle of the pinch gesture in radians (`f32`), where positive values mean counter-clockwise rotation.
/// The angle is measured in UI (screen) space, where the Y axis points downward,
/// matching typical screen coordinate conventions (not world coordinates).
///
/// When the gesture is not active, the value is `None`.
pub struct PinchRotate {
    name: Option<String>,
    is_delta: bool,
    value: Option<f32>,
}

impl PinchRotate {
    pub fn delta() -> Self {
        Self {
            name: None,
            is_delta: true,
            value: None,
        }
    }

    pub fn total() -> Self {
        Self {
            name: None,
            is_delta: false,
            value: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl UserInput for PinchRotate {
    fn type_name(&self) -> &'static str {
        "PinchRotate"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

    fn integrate(&mut self, input: &InputSources) {
        if let Some(gesture) = input.get_resource::<PinchGestureState>() {
            if let Some(positions) = gesture.positions.as_ref() {
                self.value = Some(if self.is_delta {
                    positions.delta_rotate()
                } else {
                    positions.total_rotate()
                });
            } else {
                self.value = None;
            }
        }
    }
}

impl AxisLike for PinchRotate {
    fn process(&mut self, _time: &Time) -> Option<f32> {
        self.value
    }
}

/// Represents the center point of a two-finger pinch gesture.
///
/// Returns the center point of the pinch gesture in screen coordinates (pixels),
/// with the Y axis pointing down, matching UI/screen conventions (not world coordinates).
///
/// When the gesture is not active, the value is `None`.
pub struct PinchCenter {
    name: Option<String>,
    value: Option<Vec2>,
}

impl Default for PinchCenter {
    fn default() -> Self {
        Self::new()
    }
}

impl PinchCenter {
    pub fn new() -> Self {
        Self { name: None, value: None }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl UserInput for PinchCenter {
    fn type_name(&self) -> &'static str {
        "PinchCenter"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

    fn integrate(&mut self, input: &InputSources) {
        if let Some(gesture) = input.get_resource::<PinchGestureState>() {
            if let Some(positions) = gesture.positions.as_ref() {
                self.value = Some(positions.center());
            } else {
                self.value = None;
            }
        }
    }
}

impl DualAxisLike for PinchCenter {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        self.value
    }
}
