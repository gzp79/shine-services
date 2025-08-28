use crate::{
    camera_rig::{RigDriver, RigUpdateParams},
    math::interpolate::ExpSmoothed,
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
    /// Smooth position
    pub fn position(duration_s: f32) -> Self {
        Self {
            position: ExpSmoothed::new().duration(duration_s),
            rotation: ExpSmoothed::new().duration(0.0),
        }
    }

    /// Smooth rotation
    pub fn rotation(duration_s: f32) -> Self {
        Self {
            position: ExpSmoothed::new().duration(0.0),
            rotation: ExpSmoothed::new().duration(duration_s),
        }
    }

    /// Smooth both position and rotation
    pub fn position_rotation(position_duration_s: f32, rotation_duration_s: f32) -> Self {
        Self {
            position: ExpSmoothed::new().duration(position_duration_s),
            rotation: ExpSmoothed::new().duration(rotation_duration_s),
        }
    }
}

impl RigDriver for Smooth {
    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let target_position = params.parent.translation;
        let position = self.position.exp_smooth_towards(&target_position, params.delta_time_s);

        let target_rotation = params.parent.rotation;
        let rotation = self.rotation.exp_smooth_towards(&target_rotation, params.delta_time_s);

        Transform::from_translation(position).with_rotation(rotation)
    }
}
