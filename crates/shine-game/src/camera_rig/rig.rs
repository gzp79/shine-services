use crate::{
    camera_rig::{
        camera_pose::{CameraPose, CameraPoseDebug},
        debug_camera_plugin::{DebugCameraRig, DebugCameraTarget},
        rig_driver::{RigDriver, RigDriverExt, RigUpdateParams},
        RigError,
    },
    math::value::{ValueError, ValueLike, ValueType},
};
use bevy::{
    camera::Camera,
    ecs::{
        component::Component,
        query::{With, Without},
        system::{Query, Res},
    },
    log,
    platform::collections::HashMap,
    time::Time,
    transform::components::Transform,
};

#[derive(Component)]
#[require(CameraPose)]
/// A chain of drivers, calculating displacements, and animating in succession.
pub struct CameraRig {
    drivers: Vec<Box<dyn RigDriver>>,
    parameter_map: HashMap<String, usize>,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self::new()
    }
}

impl CameraRig {
    pub fn new() -> Self {
        Self {
            drivers: Vec::new(),
            parameter_map: HashMap::new(),
        }
    }

    pub fn with<R>(mut self, driver: R) -> Result<Self, RigError>
    where
        R: RigDriver,
    {
        log::info!("Adding driver to CameraRig {}", driver.type_name());

        //the number of parameters we expect to find in the driver
        let mut count = 0;

        // check for parameter name conflicts
        let mut error: Result<(), RigError> = Ok(());
        driver.for_each_parameter(|param| {
            if let Some(name) = param.name() {
                if self.parameter_map.contains_key(name) {
                    error = Err(ValueError::DuplicateParameter(name.to_string()).into());
                    return false;
                }
                count += 1;
            }
            true
        });
        error?;

        // update parameter map with the new names
        driver.for_each_parameter(|param| {
            if let Some(name) = param.name() {
                self.parameter_map.insert(name.to_string(), self.drivers.len());
                count -= 1;
            }
            count > 0
        });
        self.drivers.push(Box::new(driver));

        Ok(self)
    }

    fn find_driver_by_parameter(&mut self, name: &str) -> Result<&mut dyn RigDriver, RigError> {
        let driver_index = self
            .parameter_map
            .get(name)
            .ok_or_else(|| ValueError::UnknownParameter(name.to_string()))?;

        Ok(&mut *self.drivers[*driver_index])
    }

    pub fn set_parameter<T>(&mut self, name: &str, value: T) -> Result<(), RigError>
    where
        T: ValueLike,
    {
        let driver = self.find_driver_by_parameter(name)?;
        driver.set_parameter(name, value)
    }

    pub fn set_parameter_with<T, F>(&mut self, name: &str, f: F) -> Result<(), RigError>
    where
        T: ValueLike,
        F: Fn(T) -> T,
    {
        let driver = self.find_driver_by_parameter(name)?;
        driver.set_parameter_with(name, f)
    }

    pub fn set_parameter_value(&mut self, name: &str, value: ValueType) -> Result<(), RigError> {
        let driver = self.find_driver_by_parameter(name)?;
        driver.set_parameter_value(name, value)
    }

    pub fn set_parameter_value_with<F>(&mut self, name: &str, f: F) -> Result<(), RigError>
    where
        F: Fn(ValueType) -> Result<ValueType, ValueError>,
    {
        let driver = self.find_driver_by_parameter(name)?;
        driver.set_parameter_value_with(name, f)
    }

    /// Runs all the drivers in sequence, animating the rig, and producing a final transform of the camera.
    pub fn calculate_transform(
        &mut self,
        delta_time_s: f32,
        mut update_steps: Option<&mut Vec<Transform>>,
    ) -> Transform {
        let mut transform = Transform::IDENTITY;

        if let Some(steps) = &mut update_steps {
            steps.clear();
        }

        for driver in self.drivers.iter_mut() {
            if let Some(steps) = &mut update_steps {
                steps.push(transform);
            }

            transform = driver.update(RigUpdateParams {
                parent: &transform,
                delta_time_s,
            });
        }

        transform
    }
}

pub fn update_camera_pose(
    camera_q: Query<(&mut CameraRig, &mut CameraPose, Option<&mut CameraPoseDebug>)>,
    time: Res<Time>,
) {
    for (mut rig, mut pose, mut debug) in camera_q {
        let update_steps = debug.as_mut().map(|d| &mut d.update_steps);
        pose.transform = rig.calculate_transform(time.delta_secs(), update_steps);
    }
}

/// Update the transformation for all the camera with a rig excluding the FreeFlyDebugCamera.
pub fn update_camera_transform(
    query: Query<(&mut Transform, &CameraPose), (With<Camera>, Without<DebugCameraTarget>)>,
) {
    for (mut transform, pose) in query {
        *transform = pose.transform;
    }
}

/// Update the transformation of the debug target
pub fn update_debug_camera_transform(
    mut camera_q: Query<&mut Transform, (With<Camera>, With<DebugCameraTarget>)>,
    pose_q: Query<&CameraPose, (With<DebugCameraRig>, Without<DebugCameraTarget>)>,
) {
    if let (Some(mut camera_transform), Some(pose)) = (camera_q.single_mut().ok(), pose_q.single().ok()) {
        *camera_transform = pose.transform;
    }
}
