use bevy::{
    app::{App, Update},
    ecs::{
        schedule::{IntoScheduleConfigs, SystemSet},
        system::ScheduleSystem,
    },
};

/// Global schedule steps within the Update stage.
#[derive(SystemSet, Clone, Hash, Debug, PartialEq, Eq)]
pub enum GameSystem {
    Input,
    Logic,
    Physics,
    Render,
}

pub trait AppGameSchedule {
    fn add_input<M>(&mut self, system: impl IntoScheduleConfigs<ScheduleSystem, M>) -> &mut Self;
    fn add_logic<M>(&mut self, system: impl IntoScheduleConfigs<ScheduleSystem, M>) -> &mut Self;
    fn add_physics<M>(&mut self, system: impl IntoScheduleConfigs<ScheduleSystem, M>) -> &mut Self;
    fn add_render<M>(&mut self, system: impl IntoScheduleConfigs<ScheduleSystem, M>) -> &mut Self;
}

impl AppGameSchedule for App {
    fn add_input<M>(&mut self, system: impl IntoScheduleConfigs<ScheduleSystem, M>) -> &mut Self {
        self.add_systems(Update, system.in_set(GameSystem::Input));
        self
    }

    fn add_logic<M>(&mut self, system: impl IntoScheduleConfigs<ScheduleSystem, M>) -> &mut Self {
        self.add_systems(Update, system.in_set(GameSystem::Logic));
        self
    }

    fn add_physics<M>(&mut self, system: impl IntoScheduleConfigs<ScheduleSystem, M>) -> &mut Self {
        self.add_systems(Update, system.in_set(GameSystem::Physics));
        self
    }

    fn add_render<M>(&mut self, system: impl IntoScheduleConfigs<ScheduleSystem, M>) -> &mut Self {
        self.add_systems(Update, system.in_set(GameSystem::Render));
        self
    }
}
