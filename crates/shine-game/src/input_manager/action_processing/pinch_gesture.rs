use bevy::{
    color::palettes::css,
    ecs::{
        component::Component,
        entity::Entity,
        system::{Local, Query, Res},
    },
    gizmos::gizmos::Gizmos,
    input::{mouse::MouseButton, touch::Touches, ButtonInput},
    math::{Isometry2d, Vec2, Vec3, Vec3Swizzles},
    render::camera::Camera,
    transform::components::GlobalTransform,
    window::Window,
};

/// Stores the screen positions of a two fingers (pinch) gesture.
#[derive(Debug, Clone)]
pub struct PinchScreenData {
    pub start: (Vec2, Vec2),
    pub prev: (Vec2, Vec2),
    pub current: (Vec2, Vec2),
}

impl PinchScreenData {
    /// Return the delta pan vector relative to the previous frame in screen coordinates (pixels).
    pub fn delta_pan(&self) -> Vec2 {
        self.center() - self.prev_center()
    }

    /// Return the total pan vector relative to the start position in screen coordinates (pixels).
    pub fn total_pan(&self) -> Vec2 {
        self.center() - self.start_center()
    }

    /// Return the delta zoom factor relative to the previous frame.
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

    /// Return the total zoom factor relative to the start position.
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

    /// Return the delta rotation angle in radians relative to the previous frame.
    /// The angle is measured in UI (screen) space, where the Y axis points downward.
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

    /// Return the total rotation angle in radians relative to the start position.
    /// The angle is measured in UI (screen) space, where the Y axis points downward.
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

    /// Return the current center point of the pinch gesture in screen coordinates (pixels).
    pub fn center(&self) -> Vec2 {
        (self.current.0 + self.current.1) / 2.0
    }

    /// Return the start center point of the pinch gesture in screen coordinates (pixels).
    pub fn start_center(&self) -> Vec2 {
        (self.start.0 + self.start.1) / 2.0
    }

    /// Return the previous center point of the pinch gesture in screen coordinates (pixels).
    pub fn prev_center(&self) -> Vec2 {
        (self.prev.0 + self.prev.1) / 2.0
    }
}

/// Stores the world positions of a two fingers (pinch) gesture.
/// Positions are usually calculated on the camera's near plane.
#[derive(Debug, Clone)]
pub struct PinchWorldData {
    pub start_center: Vec3,
    pub prev_center: Vec3,
    pub current_center: Vec3,
}

/// Resource that tracks the state of a two-finger touch gesture, including the IDs of the
/// active touch points and their positions. This is used to calculate
/// pan, zoom, and rotation deltas for multi-touch interactions.
#[derive(Debug, Clone, Component)]
pub struct TwoFingerGesture {
    camera: Option<Entity>,
    first_id: Option<u64>,
    second_id: Option<u64>,
    screen_data: Option<PinchScreenData>,
    world_data: Option<PinchWorldData>,
}

impl Default for TwoFingerGesture {
    fn default() -> Self {
        Self::new()
    }
}

impl TwoFingerGesture {
    pub fn new() -> Self {
        Self {
            camera: None,
            first_id: None,
            second_id: None,
            screen_data: None,
            world_data: None,
        }
    }

    pub fn with_camera(mut self, camera: Entity) -> Self {
        self.camera = Some(camera);
        self
    }

    pub fn screen_positions(&self) -> Option<&PinchScreenData> {
        self.screen_data.as_ref()
    }

    pub fn world_positions(&self) -> Option<&PinchWorldData> {
        self.world_data.as_ref()
    }
}

pub fn update_pinch_gesture_(
    mut gestures_q: Query<&mut TwoFingerGesture>,
    camera_q: Query<(Entity, &Camera, &GlobalTransform)>,
    touches: Res<Touches>,
) {
    for mut gesture in gestures_q.iter_mut() {
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
            if let Some(screen_data) = gesture.screen_data.as_mut() {
                screen_data.prev = screen_data.current;
                screen_data.current = (touch1.position(), touch2.position());
            } else {
                gesture.screen_data = Some(PinchScreenData {
                    start: (touch1.position(), touch2.position()),
                    prev: (touch1.position(), touch2.position()),
                    current: (touch1.position(), touch2.position()),
                });
            }
        } else {
            gesture.screen_data = None;
        }

        if let (true, Some((_, camera, camera_transform))) = (
            gesture.screen_data.is_some(),
            gesture.camera.and_then(|c| camera_q.get(c).ok()),
        ) {}
    }
}

/// Helper function to test two-finger gestures with mouse emulation
/// To update touch position:
///  - finger - 1: Left + mouse,
///  - finger - 2: Right + mouse
/// To cancel gesture:
///  - Middle button
pub fn update_pinch_gesture_emulate(
    mut gestures_q: Query<&mut TwoFingerGesture>,
    camera_q: Query<(Entity, &Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window: Query<&Window>,
    mut touch: Local<(Option<Vec2>, Option<Vec2>)>,
    mut gizmos: Gizmos,
) {
    let window = window.single().unwrap();
    let left = mouse_button.pressed(MouseButton::Left);
    let right = mouse_button.pressed(MouseButton::Right);
    let middle = mouse_button.pressed(MouseButton::Middle);
    let pos = window.cursor_position();

    if middle {
        touch.0 = None;
        touch.1 = None;
    } else if left && !right {
        touch.0 = pos;
    } else if right && !left {
        touch.1 = pos;
    }

    for (_, camera, camera_transform) in camera_q.iter() {
        if !camera.is_active {
            continue;
        }

        if let Some(origin) = touch
            .0
            .and_then(|p| camera.viewport_to_world_2d(camera_transform, p).ok())
        {
            gizmos.circle_2d(Isometry2d::from_translation(origin), 10., css::RED);
        }
        if let Some(origin) = touch
            .1
            .and_then(|p| camera.viewport_to_world_2d(camera_transform, p).ok())
        {
            gizmos.circle_2d(Isometry2d::from_translation(origin), 10., css::GREEN);
        }
    }

    for mut gesture in gestures_q.iter_mut() {
        if touch.0.is_some() {
            gesture.first_id = Some(0);
        } else {
            gesture.first_id = None;
        }

        if touch.1.is_some() {
            gesture.second_id = Some(1);
        } else {
            gesture.second_id = None;
        }

        if let (Some(touch1), Some(touch2)) = (touch.0, touch.1) {
            if let Some(screen_data) = gesture.screen_data.as_mut() {
                screen_data.prev = screen_data.current;
                screen_data.current = (touch1, touch2);
            } else {
                gesture.screen_data = Some(PinchScreenData {
                    start: (touch1, touch2),
                    prev: (touch1, touch2),
                    current: (touch1, touch2),
                });
            }
        } else {
            gesture.screen_data = None;
        }

        if let (true, Some((_, camera, camera_transform))) = (
            gesture.screen_data.is_some(),
            gesture.camera.and_then(|c| camera_q.get(c).ok()),
        ) {}
    }
}
