use crate::{
    camera_rig::{CameraPose, RigDriver},
    math::value::{ExpSmoothed, Variable},
};
use bevy::math::{Quat, Vec3};

/// Predict the parent transformation. Similar to smooth it overshots the target and then smooths to the target.
pub struct Predict {
    position: ExpSmoothed<Vec3>,
    rotation: ExpSmoothed<Quat>,
}

impl Default for Predict {
    fn default() -> Self {
        Self::position_rotation(1.0, 1.0)
    }
}

impl Predict {
    /// Predict both position and rotation
    pub fn position_rotation(position_duration_s: f32, rotation_duration_s: f32) -> Self {
        // Initialize without a current value to allow smooth prediction from the first update.
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

impl RigDriver for Predict {
    fn visit_variables(&self, _visitor: &mut dyn FnMut(&dyn Variable) -> bool) {}

    fn variable_mut(&mut self, _name: &str) -> Option<&mut dyn Variable> {
        None
    }

    fn update(&mut self, pose: &mut CameraPose, delta_time_s: f32) {
        let target_position = pose.transform.translation;
        pose.transform.translation = self.position.predict_from(&target_position, delta_time_s);

        let target_rotation = pose.transform.rotation;
        pose.transform.rotation = self.rotation.predict_from(&target_rotation, delta_time_s);
    }
}
