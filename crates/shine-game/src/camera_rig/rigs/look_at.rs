use crate::camera_rig::{rigs::ExpSmoothed, RigDriver, RigUpdateParams};
use bevy::{math::Vec3, transform::components::Transform};

/// Rotates the camera to point at a world-space position.
///
/// The target tracking can be additionally smoothed, and made to look ahead of it.
pub struct LookAt {
    pub target: Vec3,
    smoothed_target: ExpSmoothed<Vec3>,
    predictive: bool,
}

impl LookAt {
    pub fn new<P>(target: P) -> Self
    where
        P: Into<Vec3>,
    {
        let target = target.into();

        Self {
            target,
            smoothed_target: Default::default(),
            predictive: false,
        }
    }

    /// Set the exponential smoothing factor for target position tracking.
    pub fn smoothness(self, smoothness: f32) -> Self {
        Self {
            smoothed_target: self.smoothed_target.smoothness(smoothness),
            ..self
        }
    }

    pub fn predictive(mut self, predictive: bool) -> Self {
        self.predictive = predictive;
        self
    }
}

impl RigDriver for LookAt {
    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let target = if self.predictive {
            self.smoothed_target.exp_predict_from(&self.target, params.delta_time_s)
        } else {
            self.smoothed_target
                .exp_smooth_towards(&self.target, params.delta_time_s)
        };

        let parent_position = params.parent.translation;
        Transform::from_translation(parent_position).looking_at(target, Vec3::Y)
    }
}
