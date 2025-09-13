use crate::bevy_ext::CameraExt;
use bevy::{
    color::palettes::css,
    ecs::{
        component::Component,
        entity::Entity,
        system::{Local, Query, Res},
    },
    gizmos::gizmos::Gizmos,
    input::{mouse::MouseButton, touch::Touches, ButtonInput},
    math::{ops::atan2, Isometry2d, Mat2, Quat, Vec2, Vec3},
    render::camera::Camera,
    transform::components::{GlobalTransform, Transform},
    window::Window,
};

// disable touch update when the distance between two touches is too small
const MIN_PIXEL_DISTANCE: f32 = 5.0;
const EPS: f32 = 1e-6;

/// Stores the screen positions of a two fingers (pinch) gesture.
#[derive(Debug, Clone)]
pub struct PinchData {
    pub start: (Vec2, Vec2),
    pub prev: (Vec2, Vec2),
    pub current: (Vec2, Vec2),
}

impl PinchData {
    /// Return the delta pan vector in screen coordinates (pixels).
    pub fn pan(&self, from_start: bool) -> Vec2 {
        let start = if from_start {
            self.start_center()
        } else {
            self.prev_center()
        };

        self.center() - start
    }

    /// Return the delta zoom factor in screen coordinates (pixels).
    pub fn zoom(&self, from_start: bool) -> f32 {
        let start = if from_start { self.start } else { self.prev };

        let prev = (start.1 - start.0).length();
        let current = (self.current.1 - self.current.0).length();

        // For degenerate cases, return no-zoom
        if prev < EPS || current < EPS {
            1.0
        } else {
            current / prev
        }
    }

    /// Return the delta rotation angle in radians measured in UI (screen) space, where the Y axis points downward.
    pub fn rotate(&self, from_start: bool) -> f32 {
        let start = if from_start { self.start } else { self.prev };

        let prev = start.1 - start.0;
        let current = self.current.1 - self.current.0;

        // For degenerate cases, return no-rotation
        if prev.length_squared() < EPS || current.length_squared() < EPS {
            0.0
        } else {
            atan2(current.y, current.x) - atan2(prev.y, prev.x)
        }
    }

    /// Return the start center point of the pinch gesture in screen coordinates (pixels).
    pub fn start_center(&self) -> Vec2 {
        (self.start.0 + self.start.1) / 2.0
    }

    /// Return the previous center point of the pinch gesture in screen coordinates (pixels).
    pub fn prev_center(&self) -> Vec2 {
        (self.prev.0 + self.prev.1) / 2.0
    }

    /// Return the current center point of the pinch gesture in screen coordinates (pixels).
    pub fn center(&self) -> Vec2 {
        (self.current.0 + self.current.1) / 2.0
    }

    /// Convert the pinch data to screen-centered coordinates using the provided camera.
    pub fn viewport_to_screen_centered(&self, camera: &Camera) -> Option<Self> {
        Some(Self {
            start: (
                camera.viewport_to_screen_centered(self.start.0).ok()?,
                camera.viewport_to_screen_centered(self.start.1).ok()?,
            ),
            prev: (
                camera.viewport_to_screen_centered(self.prev.0).ok()?,
                camera.viewport_to_screen_centered(self.prev.1).ok()?,
            ),
            current: (
                camera.viewport_to_screen_centered(self.current.0).ok()?,
                camera.viewport_to_screen_centered(self.current.1).ok()?,
            ),
        })
    }
}

/// Component tracking the state of a two-finger touch gesture, including the IDs of the
/// active touch points and their positions. This is used to calculate
/// pan, zoom, and rotation deltas for multi-touch interactions.
#[derive(Debug, Clone, Component)]
pub struct TwoFingerGesture {
    first_id: Option<u64>,
    second_id: Option<u64>,
    screen_data: Option<PinchData>,
}

impl Default for TwoFingerGesture {
    fn default() -> Self {
        Self::new()
    }
}

impl TwoFingerGesture {
    pub fn new() -> Self {
        Self {
            first_id: None,
            second_id: None,
            screen_data: None,
        }
    }

    pub fn screen_data(&self) -> Option<&PinchData> {
        self.screen_data.as_ref()
    }

    /// Find the new camera transformation based on the pinch gesture data for un-scaled orthographic projection.
    /// It is assumed the viewport and projection matrices are not changed during a pinch gesture.
    pub fn transform_view_2d(
        &self,
        camera: &Camera,
        camera_transform: &Transform,
        from_start: bool,
    ) -> Option<Transform> {
        if let Some(screen) = self
            .screen_data
            .as_ref()
            .and_then(|s| s.viewport_to_screen_centered(camera))
        {
            let s = screen.zoom(from_start);
            let phi = screen.rotate(from_start);
            let t = {
                let rot = Mat2::from_angle(phi) * s;
                let p1 = if from_start { screen.start.1 } else { screen.prev.1 };
                screen.current.1 - rot * p1
            };

            let (s, phi, t) = {
                let inv_s = 1.0 / s;
                let inv_phi = -phi;
                let inv_rot = Mat2::from_angle(inv_phi) * inv_s;
                let inv_t = -(inv_rot * t);
                (inv_s, inv_phi, inv_t)
            };

            let delta = Transform {
                translation: t.extend(0.0),
                rotation: Quat::from_rotation_z(phi),
                scale: Vec3::splat(s),
            };

            Some(*camera_transform * delta)
        } else {
            None
        }
    }

    fn update_screen_data(&mut self, touch1: Vec2, touch2: Vec2) {
        if touch1.distance_squared(touch2) < MIN_PIXEL_DISTANCE {
            // If the two touches are too close, treat them as the same point
            return;
        }
        if let Some(screen_data) = self.screen_data.as_mut() {
            screen_data.prev = screen_data.current;
            screen_data.current = (touch1, touch2);
        } else {
            self.screen_data = Some(PinchData {
                start: (touch1, touch2),
                prev: (touch1, touch2),
                current: (touch1, touch2),
            });
        }
    }
}

pub fn update_pinch_gesture(mut gestures_q: Query<&mut TwoFingerGesture>, touches: Res<Touches>) {
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

        // Update screen data based on the current touch positions
        if let (Some(touch1), Some(touch2)) = (
            gesture.first_id.and_then(|id| touches.get_pressed(id)),
            gesture.second_id.and_then(|id| touches.get_pressed(id)),
        ) {
            gesture.update_screen_data(touch1.position(), touch2.position());
        } else {
            gesture.screen_data = None;
        }
    }
}

/// Helper function to test two-finger gestures with mouse emulation
/// Updating touch position:
///  - finger - 1: Left + mouse,
///  - finger - 2: Right + mouse
///
/// Canceling gesture:
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
    //let pos = pos.map(|pos| Vec2::new(pos.x, window.height() - pos.y));

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

        // Assuming uniform scale for 2D camera
        let s = camera_transform.to_scale_rotation_translation().0.x;

        if let Some(origin) = touch
            .0
            .and_then(|p| camera.viewport_to_world_2d(camera_transform, p).ok())
        {
            gizmos.circle_2d(Isometry2d::from_translation(origin), 4. * s, css::RED);
        }
        if let Some(origin) = touch
            .1
            .and_then(|p| camera.viewport_to_world_2d(camera_transform, p).ok())
        {
            gizmos.circle_2d(Isometry2d::from_translation(origin), 4. * s, css::GREEN);
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

        // Update screen data based on the current touch positions
        if let (Some(touch1), Some(touch2)) = (touch.0, touch.1) {
            gesture.update_screen_data(touch1, touch2);
        } else {
            gesture.screen_data = None;
        }
    }
}
