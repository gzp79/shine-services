use crate::camera_rig::{RigDriver, RigError, RigParameter, RigUpdateParams, ValueType};
use bevy::{
    math::{EulerRot, Quat},
    transform::components::Transform,
};

/// Calculate camera rotation based on yaw and pitch angles.
pub struct YawPitch<Y, P>
where
    Y: RigParameter<Value = f32>,
    P: RigParameter<Value = f32>,
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
    Y: RigParameter<Value = f32>,
    P: RigParameter<Value = f32>,
{
    pub fn new(yaw: Y, pitch: P) -> Self {
        Self { yaw, pitch }
    }
}

impl<Y, P> RigDriver for YawPitch<Y, P>
where
    Y: RigParameter<Value = f32>,
    P: RigParameter<Value = f32>,
{
    fn parameter_names(&self) -> Vec<&str> {
        (self.yaw.name().into_iter()).chain(self.pitch.name()).collect()
    }

    fn set_parameter_value(&mut self, name: &str, value: ValueType) -> Result<(), RigError> {
        if self.yaw.name() == Some(name) {
            self.yaw.set(f32::try_from(value)? % 720.0);
            Ok(())
        } else if self.pitch.name() == Some(name) {
            self.pitch.set(f32::try_from(value)?.clamp(-90.0, 90.0));
            Ok(())
        } else {
            Err(RigError::UnknownParameter(name.into()))
        }
    }

    fn get_parameter_value(&self, name: &str) -> Result<ValueType, RigError> {
        if self.yaw.name() == Some(name) {
            Ok((*self.yaw.get()).into())
        } else if self.pitch.name() == Some(name) {
            Ok((*self.pitch.get()).into())
        } else {
            Err(RigError::UnknownParameter(name.into()))
        }
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let yaw = self.yaw.update(params.delta_time_s) % 720_f32;
        let pitch = self.pitch.update(params.delta_time_s).clamp(-90.0, 90.0);

        let rotation = Quat::from_euler(EulerRot::YXZ, yaw.to_radians(), pitch.to_radians(), 0.0);

        Transform::from_translation(params.parent.translation).with_rotation(rotation)
    }
}
