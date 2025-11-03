use crate::{
    camera_rig::{CameraPose, RigDriver},
    math::value::{ExpSmoothed, Variable},
};
use bevy::math::{Quat, Vec3};

/// Smooths the parent transformation.
pub struct Smooth {
    position: ExpSmoothed<Vec3>,
    rotation: ExpSmoothed<Quat>,
}

impl Default for Smooth {
    fn default() -> Self {
        Self::position_rotation(1.0, 1.0)
    }
}

impl Smooth {
    /// Predict both position and rotation
    pub fn position_rotation(position_duration_s: f32, rotation_duration_s: f32) -> Self {
        // Initialize without a current value to allow smooth transitions from the first update.
        // The initial state will be set when the first target value is provided.
        Self {
            position: ExpSmoothed::new(position_duration_s, None),
            rotation: ExpSmoothed::new(rotation_duration_s, None),
        }
    }

    /// Predict position
    pub fn position(duration_s: f32) -> Self {
        Self::position_rotation(duration_s, 0.0)
    }

    /// Predict rotation
    pub fn rotation(duration_s: f32) -> Self {
        Self::position_rotation(0.0, duration_s)
    }
}

impl RigDriver for Smooth {
    fn visit_variables(&self, _visitor: &mut dyn FnMut(&dyn Variable) -> bool) {}

    fn variable_mut(&mut self, _name: &str) -> Option<&mut dyn Variable> {
        None
    }

    fn update(&mut self, pose: &mut CameraPose, delta_time_s: f32) {
        let target_position = pose.transform.translation;
        pose.transform.translation = self.position.smooth_towards(&target_position, delta_time_s);

        let target_rotation = pose.transform.rotation;
        pose.transform.rotation = self.rotation.smooth_towards(&target_rotation, delta_time_s);
    }
}
