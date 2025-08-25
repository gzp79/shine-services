use crate::camera_rig::{AnyRigDriver, CameraPose, DebugCameraPose, DebugTargetCamera, RigDriver, RigUpdateParams};
use bevy::{
    ecs::{
        component::Component,
        query::{With, Without},
        system::{Query, Res},
    },
    render::camera::Camera,
    time::Time,
    transform::components::Transform,
};
use itertools::Itertools;

#[derive(Component)]
#[require(CameraPose)]
/// A chain of drivers, calculating displacements, and animating in succession.
pub struct CameraRig {
    pub drivers: Vec<Box<dyn AnyRigDriver>>,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self::new()
    }
}

impl CameraRig {
    pub fn new() -> Self {
        Self { drivers: Vec::new() }
    }

    pub fn with<R>(mut self, driver: R) -> Self
    where
        R: AnyRigDriver,
    {
        self.drivers.push(Box::new(driver));
        self
    }

    /// Returns the driver of the matching type.
    /// ## Panics
    /// If multiple or no driver of the matching type is present.
    pub fn driver_mut<T: RigDriver>(&mut self) -> &mut T {
        self.try_driver_mut::<T>()
            .unwrap_or_else(|| panic!("No {} driver found in the CameraRig", std::any::type_name::<T>()))
    }

    /// Returns the driver of the matching type, or `None` if no such driver is present.
    /// ## Panics
    /// If no driver of the matching type is present, panics.
    pub fn try_driver_mut<T: RigDriver>(&mut self) -> Option<&mut T> {
        self.drivers
            .iter_mut()
            .filter_map(|driver| driver.as_mut().as_any_mut().downcast_mut::<T>())
            .at_most_one()
            .unwrap_or_else(|_| panic!("Multiple {} driver found in the CameraRig", std::any::type_name::<T>()))
    }

    /// Returns the driver of the matching type.
    /// ## Panics
    /// If multiple or no driver of the matching type is present.
    pub fn driver<T: RigDriver>(&self) -> &T {
        self.try_driver::<T>()
            .unwrap_or_else(|| panic!("No {} driver found in the CameraRig", std::any::type_name::<T>()))
    }

    /// Returns the driver of the matching type, or `None` if no such driver is present.
    /// ## Panics
    /// If no driver of the matching type is present, panics.
    pub fn try_driver<T: RigDriver>(&self) -> Option<&T> {
        self.drivers
            .iter()
            .filter_map(|driver| driver.as_ref().as_any().downcast_ref::<T>())
            .at_most_one()
            .unwrap_or_else(|_| panic!("Multiple {} driver found in the CameraRig", std::any::type_name::<T>()))
    }

    /// Runs all the drivers in sequence, animating the rig, and producing a final transform of the camera.
    pub fn calculate_transform(&mut self, delta_time_s: f32) -> Transform {
        let mut transform = Transform::IDENTITY;

        for driver in self.drivers.iter_mut() {
            transform = driver.update(RigUpdateParams {
                parent: &transform,
                delta_time_s,
            });
        }

        transform
    }
}

pub fn update_camera_pose(camera_q: Query<(&mut CameraRig, &mut CameraPose)>, time: Res<Time>) {
    for (mut rig, mut pose) in camera_q {
        pose.transform = rig.calculate_transform(time.delta_secs());
    }
}

/// Update the transformation for all the camera with a rig excluding the DebugTargetCamera.
pub fn update_camera_transform(
    query: Query<(&mut Transform, &CameraPose), (With<Camera>, Without<DebugTargetCamera>)>,
) {
    for (mut transform, pose) in query {
        *transform = pose.transform;
    }
}

/// Update the transformation of the debug target
pub fn update_debug_camera_transform(
    mut camera_q: Query<&mut Transform, (With<Camera>, With<DebugTargetCamera>)>,
    pose_q: Query<&CameraPose, (With<DebugCameraPose>, Without<DebugTargetCamera>)>,
) {
    bevy::log::debug!("Has target: {}, pose: {}", camera_q.is_empty(), pose_q.is_empty());
    if let (Some(mut camera_transform), Some(pose)) = (camera_q.single_mut().ok(), pose_q.single().ok()) {
        *camera_transform = pose.transform;
    }
}
