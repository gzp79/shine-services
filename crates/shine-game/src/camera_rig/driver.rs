use bevy::transform::components::Transform;

pub struct RigUpdateParams<'a> {
    pub parent: &'a Transform,
    pub delta_time_s: f32,
}

/// A building block of a camera rig, to calculate the transform of the camera.
pub trait RigDriver: std::any::Any {
    /// Calculates the transform of this driver component based on the parent
    /// provided in `params`.
    fn update(&mut self, params: RigUpdateParams) -> Transform;
}

/// A utility trait for all camera rig drivers to help with type erasure and dynamic dispatch.
pub trait AnyRigDriver: RigDriver + Sync + Send + std::any::Any {
    /// Returns `self` as `&dyn Any`
    fn as_any(&self) -> &dyn std::any::Any;

    /// Returns `self` as `&mut dyn Any`
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T> AnyRigDriver for T
where
    T: RigDriver + std::any::Any + Sync + Send,
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
