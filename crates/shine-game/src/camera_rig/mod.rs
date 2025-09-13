//! Camera rig module based on https://github.com/h3r2tic/dolly/tree/main

mod camera_pose;
mod debug_camera_plugin;
mod rig;
mod rig_driver;
mod rig_error;
mod rig_plugin;

pub mod rigs;

pub use self::{
    camera_pose::{CameraPose, CameraPoseDebug},
    debug_camera_plugin::DebugCameraTarget,
    rig::CameraRig,
    rig_driver::{RigDriver, RigUpdateParams},
    rig_error::RigError,
    rig_plugin::CameraRigPlugin,
};
