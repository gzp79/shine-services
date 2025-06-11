use crate::camera_rig::{drivers::ExpSmoothed, RigDriver, RigUpdateParams};
use bevy::{
    math::{Quat, Vec3},
    transform::components::Transform,
};

/// Smooths the parent transformation.
pub struct Smooth {
    smoothed_position: ExpSmoothed<Vec3>,
    smoothed_rotation: ExpSmoothed<Quat>,
}

impl Default for Smooth {
    fn default() -> Self {
        Self::new_position_rotation(1.0, 1.0)
    }
}

impl Smooth {
    /// Only smooth position
    pub fn new_position(position_smoothness: f32) -> Self {
        Self {
            smoothed_position: ExpSmoothed::new().smoothness(position_smoothness),
            smoothed_rotation: ExpSmoothed::new().smoothness(0.0),
        }
    }

    /// Only smooth rotation
    pub fn new_rotation(rotation_smoothness: f32) -> Self {
        Self {
            smoothed_position: ExpSmoothed::new().smoothness(0.0),
            smoothed_rotation: ExpSmoothed::new().smoothness(rotation_smoothness),
        }
    }

    /// Smooth both position and rotation
    pub fn new_position_rotation(position_smoothness: f32, rotation_smoothness: f32) -> Self {
        Self {
            smoothed_position: ExpSmoothed::new().smoothness(position_smoothness),
            smoothed_rotation: ExpSmoothed::new().smoothness(rotation_smoothness),
        }
    }

    /// Reverse the smoothing, causing the camera to look ahead of the parent transform
    ///
    /// This can be useful on top of [`Position`], and before another `Smooth`
    /// in the chain to create a soft yet responsive follower camera.
    pub fn predictive(self, predictive: bool) -> Self {
        Self {
            smoothed_position: self.smoothed_position.predictive(predictive),
            smoothed_rotation: self.smoothed_rotation.predictive(predictive),
        }
    }
}

impl RigDriver for Smooth {
    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let target_position = params.parent.translation;
        let position = self
            .smoothed_position
            .exp_smooth_towards(&target_position, params.delta_time);

        let target_rotation = params.parent.rotation;
        let rotation = self
            .smoothed_rotation
            .exp_smooth_towards(&target_rotation, params.delta_time);

        Transform::from_translation(position).with_rotation(rotation)
    }
}
