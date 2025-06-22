use crate::camera_rig::{RigDriver, RigUpdateParams};
use bevy::{
    math::{EulerRot, Quat},
    transform::components::Transform,
};

/// Calculate camera rotation based on yaw and pitch angles.
pub struct YawPitch {
    /// [0..720)
    ///
    /// Note: Quaternions can encode 720 degrees of rotation, causing a slerp from 350 to 0 degrees
    /// to happen counter-intuitively in the negative direction; the positive direction would go through 720,
    /// thus being farther. By encoding rotation here in the 0..720 range, we reduce the risk of this happening.
    pub yaw_degrees: f32,

    /// [-90..90]
    pub pitch_degrees: f32,
}

impl Default for YawPitch {
    fn default() -> Self {
        Self::new()
    }
}

impl YawPitch {
    /// Creates camera looking forward along Z axis (negative or positive depends on system handedness)
    pub fn new() -> Self {
        Self {
            yaw_degrees: 0.0,
            pitch_degrees: 0.0,
        }
    }

    /// Set the yaw angle in degrees.
    pub fn yaw_degrees(mut self, yaw_degrees: f32) -> Self {
        self.yaw_degrees = yaw_degrees % 720_f32;
        self
    }

    /// Set the pitch angle in degrees.
    pub fn pitch_degrees(mut self, pitch_degrees: f32) -> Self {
        self.pitch_degrees = pitch_degrees.clamp(-90.0, 90.0);
        self
    }

    /// Additively rotate by the specified angles.
    pub fn rotate_yaw_pitch(&mut self, yaw_degrees: f32, pitch_degrees: f32) {
        self.yaw_degrees = (self.yaw_degrees + yaw_degrees) % 720_f32;
        self.pitch_degrees = (self.pitch_degrees + pitch_degrees).clamp(-90.0, 90.0);
    }
}

impl RigDriver for YawPitch {
    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let yaw = self.yaw_degrees % 720_f32;
        let pitch = self.pitch_degrees.clamp(-90.0, 90.0);

        let rotation = Quat::from_euler(EulerRot::YXZ, yaw.to_radians(), pitch.to_radians(), 0.0);

        Transform::from_translation(params.parent.translation).with_rotation(rotation)
    }
}
