use crate::input_manager::{ActionLike, AxisLike, DualAxisLike, InputMap, InputSource, InputSources, UserInput};
use bevy::{
    ecs::{
        resource::Resource,
        system::{Query, Res, ResMut},
    },
    input::touch::Touches,
    math::Vec2,
    time::Time,
    window::Window,
};

/// Positional information for the two touch fingers.
#[derive(Debug, Clone)]
pub struct TwoFingerPositions {
    pub start: (Vec2, Vec2),
    pub prev: (Vec2, Vec2),
    pub current: (Vec2, Vec2),
}

/// Handle two-finger touch gestures by tracking the state of (the first) two fingers on the screen.
#[derive(Debug, Clone, Default, Resource)]
pub struct TwoFingerTouchGesture {
    pub first_id: Option<u64>,
    pub second_id: Option<u64>,
    pub positions: Option<TwoFingerPositions>,
}

pub fn update_two_finger_touch_gesture(mut gesture: ResMut<TwoFingerTouchGesture>, touches: Res<Touches>) {
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
        gesture.positions = Some(TwoFingerPositions {
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
    gesture: Res<TwoFingerTouchGesture>,
    mut input_query: Query<&mut InputMap<A>>,
) where
    A: ActionLike,
{
    let window = window.single().expect("Only single window is supported");

    for mut input_map in input_query.iter_mut() {
        let mut input_source = InputSources::new();

        input_source.add_resource(window);
        input_source.add_resource(&*time);
        input_source.add_resource(&*gesture);

        input_map.integrate(input_source);
    }
}

impl InputSource for TwoFingerTouchGesture {}

/// Return pinch pan based on the two-finger touch gesture.
pub struct PinchPan {
    value: Option<Vec2>,
}

impl Default for PinchPan {
    fn default() -> Self {
        Self::new()
    }
}

impl PinchPan {
    pub fn new() -> Self {
        Self { value: None }
    }
}

impl UserInput for PinchPan {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(positions) = input
            .get_resource::<TwoFingerTouchGesture>()
            .and_then(|g| g.positions.as_ref())
        {
            let prev = (positions.prev.0 + positions.prev.1) / 2.0;
            let current = (positions.current.0 + positions.current.1) / 2.0;
            self.value = Some(current - prev);
        }
    }
}

impl DualAxisLike for PinchPan {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        self.value
    }
}

/// Return pinch zoom based on the two-finger touch gesture.
pub struct PinchZoom {
    value: Option<f32>,
}

impl Default for PinchZoom {
    fn default() -> Self {
        Self::new()
    }
}

impl PinchZoom {
    pub fn new() -> Self {
        Self { value: None }
    }
}

impl UserInput for PinchZoom {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(positions) = input
            .get_resource::<TwoFingerTouchGesture>()
            .and_then(|g| g.positions.as_ref())
        {
            let prev = (positions.prev.1 - positions.prev.0).length();
            let current = (positions.current.1 - positions.current.0).length();

            // touches at the same pixel position is considered as no zoom
            if prev > 1.0 {
                self.value = Some(current / prev);
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

/// Return pinch rotate based on the two-finger touch gesture.
pub struct PinchRotate {
    value: Option<f32>,
}

impl Default for PinchRotate {
    fn default() -> Self {
        Self::new()
    }
}

impl PinchRotate {
    pub fn new() -> Self {
        Self { value: None }
    }
}

impl UserInput for PinchRotate {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(positions) = input
            .get_resource::<TwoFingerTouchGesture>()
            .and_then(|g| g.positions.as_ref())
        {
            let prev = positions.prev.1 - positions.prev.0;
            let current = positions.current.1 - positions.current.0;

            if prev.length_squared() < 1.0 || current.length_squared() < 1.0 {
                self.value = None;
            } else {
                self.value = Some(prev.angle_to(current));
            }
        }
    }
}

impl AxisLike for PinchRotate {
    fn process(&mut self, _time: &Time) -> Option<f32> {
        self.value
    }
}
