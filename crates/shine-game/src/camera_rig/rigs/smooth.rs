use crate::{
    camera_rig::{RigDriver, RigError, RigUpdateParams},
    math::temporal::{ExpSmoothed, ValueError, ValueType},
};
use bevy::{
    math::{Quat, Vec3},
    transform::components::Transform,
};

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
    fn parameter_names(&self) -> Vec<&str> {
        vec![]
    }

    fn set_parameter_value(&mut self, name: &str, _value: ValueType) -> Result<(), RigError> {
        Err(ValueError::UnknownParameter(name.into()).into())
    }

    fn get_parameter_value(&self, name: &str) -> Result<ValueType, RigError> {
        Err(ValueError::UnknownParameter(name.into()).into())
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let target_position = params.parent.translation;
        let position = self.position.smooth_towards(&target_position, params.delta_time_s);

        let target_rotation = params.parent.rotation;
        let rotation = self.rotation.smooth_towards(&target_rotation, params.delta_time_s);

        Transform::from_translation(position).with_rotation(rotation)
    }
}
