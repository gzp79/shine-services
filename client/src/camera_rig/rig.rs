use crate::camera_rig::{RigDriver, RigDriverTraits, RigUpdateParams};
use bevy::{ecs::component::Component, transform::components::Transform};
use itertools::Itertools;

#[derive(Component)]
/// A chain of drivers, calculating displacements, and animating in succession.
pub struct CameraRig {
    pub drivers: Vec<Box<dyn RigDriverTraits>>,
    pub final_transform: Transform,
}

impl CameraRig {
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
    pub fn update(&mut self, delta_time: f32) -> Transform {
        let mut parent_transform = Transform::IDENTITY;

        for driver in self.drivers.iter_mut() {
            let transform = driver.update(RigUpdateParams {
                parent: &parent_transform,
                delta_time,
            });

            parent_transform = transform;
        }

        self.final_transform = parent_transform;
        self.final_transform
    }

    /// Returns the transform of the last update.
    pub fn transform(&self) -> &Transform {
        &self.final_transform
    }

    /// Use this to make a new rig
    pub fn builder() -> CameraRigBuilder {
        CameraRigBuilder { drivers: Default::default() }
    }
}

pub struct CameraRigBuilder {
    drivers: Vec<Box<dyn RigDriverTraits>>,
}

impl CameraRigBuilder {
    pub fn with<R>(mut self, driver: R) -> Self
    where
        R: RigDriverTraits,
    {
        self.drivers.push(Box::new(driver));
        self
    }

    pub fn build(self) -> CameraRig {
        let mut rig = CameraRig {
            drivers: self.drivers,
            final_transform: Transform::IDENTITY,
        };

        // Initialize the rig by updating it with a delta time of 0.0
        rig.update(0.0);
        rig
    }
}
