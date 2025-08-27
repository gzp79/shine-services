use crate::camera_rig::{rigs::ExpSmoothed, RigDriver, RigUpdateParams};
use bevy::{
    math::{Quat, Vec3},
    transform::components::Transform,
};

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
    /// Predict position
    pub fn position(position_smoothness: f32) -> Self {
        Self {
            position: ExpSmoothed::new().smoothness(position_smoothness),
            rotation: ExpSmoothed::new().smoothness(0.0),
        }
    }

    /// Predict rotation
    pub fn rotation(rotation_smoothness: f32) -> Self {
        Self {
            position: ExpSmoothed::new().smoothness(0.0),
            rotation: ExpSmoothed::new().smoothness(rotation_smoothness),
        }
    }

    /// Predict both position and rotation
    pub fn position_rotation(position_smoothness: f32, rotation_smoothness: f32) -> Self {
        Self {
            position: ExpSmoothed::new().smoothness(position_smoothness),
            rotation: ExpSmoothed::new().smoothness(rotation_smoothness),
        }
    }
}

impl RigDriver for Predict {
    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let target_position = params.parent.translation;
        let position = self.position.exp_predict_from(&target_position, params.delta_time_s);

        let target_rotation = params.parent.rotation;
        let rotation = self.rotation.exp_predict_from(&target_rotation, params.delta_time_s);

        Transform::from_translation(position).with_rotation(rotation)
    }
}
