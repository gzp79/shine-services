use bevy::{
    app::{App, Update},
    ecs::{
        schedule::{IntoScheduleConfigs, SystemSet},
        system::ScheduleSystem,
    },
};

/// A trait for all abstract systems for fine grained scheduling in the Update stage.
pub trait UpdateSystem: SystemSet + Sized {}

/// Global schedule steps within the Update stage.
#[derive(SystemSet, Clone, Hash, Debug, PartialEq, Eq)]
pub enum GameSystems {
    Action,
    PrepareSimulate,
    Simulate,
    PrepareRender,
}

impl UpdateSystem for GameSystems {}

/// Fine grained PrepareSimulate steps for camera handling
#[derive(SystemSet, Clone, Hash, Debug, PartialEq, Eq)]
pub enum CameraSimulate {
    PreparePose,
    SimulatePose,
    WithPose,
}

impl UpdateSystem for CameraSimulate {}

pub trait AppGameSchedule {
    fn add_update_systems<M>(
        &mut self,
        set: impl UpdateSystem,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;
}

impl AppGameSchedule for App {
    fn add_update_systems<M>(
        &mut self,
        set: impl UpdateSystem,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.add_systems(Update, system.in_set(set));
        self
    }
}
