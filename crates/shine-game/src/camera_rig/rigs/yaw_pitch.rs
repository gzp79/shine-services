use crate::{
    camera_rig::{RigDriver, RigUpdateParams},
    math::value::{AnimatedVariable, Variable},
};
use bevy::{
    log,
    math::{EulerRot, Quat},
    transform::components::Transform,
};

/// Calculate camera rotation based on yaw and pitch angles.
pub struct YawPitch<Y, P>
where
    Y: Variable + AnimatedVariable<Value = f32>,
    P: Variable + AnimatedVariable<Value = f32>,
{
    /// [0..720)
    ///
    /// Note: Quaternions can encode 720 degrees of rotation, causing a slerp from 350 to 0 degrees
    /// to happen counter-intuitively in the negative direction; the positive direction would go through 720,
    /// thus being farther. By encoding rotation here in the 0..720 range, we reduce the risk of this happening.
    pub yaw: Y,

    /// [-90..90]
    pub pitch: P,
}

impl Default for YawPitch<f32, f32> {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

impl<Y, P> YawPitch<Y, P>
where
    Y: Variable + AnimatedVariable<Value = f32>,
    P: Variable + AnimatedVariable<Value = f32>,
{
    pub fn new(yaw: Y, pitch: P) -> Self {
        Self { yaw, pitch }
    }
}

impl<Y, P> RigDriver for YawPitch<Y, P>
where
    Y: Variable + AnimatedVariable<Value = f32>,
    P: Variable + AnimatedVariable<Value = f32>,
{
    fn visit_parameters(&self, visitor: &mut dyn FnMut(&dyn Variable) -> bool) {
        visitor(&self.yaw);
        visitor(&self.pitch);
    }

    fn parameter_mut(&mut self, name: &str) -> Option<&mut dyn Variable> {
        if self.yaw.name() == Some(name) {
            Some(&mut self.yaw)
        } else if self.pitch.name() == Some(name) {
            Some(&mut self.pitch)
        } else {
            None
        }
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let yaw = self.yaw.animate(params.delta_time_s);

        const YAW_PRECISION_THRESHOLD: f32 = 1440.0; // ~4 full rotations
        if yaw.abs() > YAW_PRECISION_THRESHOLD {
            log::warn!("Yaw exceeds safe rotation range, consider normalizing: {yaw}");
        }
        let yaw = yaw % 720_f32;

        let pitch = self.pitch.animate(params.delta_time_s);

        const PITCH_PRECISION_THRESHOLD: f32 = 270.0; // ~3 full rotations worth
        if pitch.abs() > PITCH_PRECISION_THRESHOLD {
            log::warn!("Pitch exceeds reasonable range, consider normalizing: {pitch}");
        }
        let pitch = pitch.clamp(-90.0, 90.0);

        let rotation = Quat::from_euler(EulerRot::YXZ, yaw.to_radians(), pitch.to_radians(), 0.0);

        Transform::from_translation(params.parent.translation).with_rotation(rotation)
    }
}
