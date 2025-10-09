use bevy::{
    ecs::schedule::{Chain, GraphInfo, IntoScheduleConfigs, Schedulable, ScheduleConfigs},
    state::{condition::in_state, state::States},
};

pub trait ScheduleExt<T, Marker>: IntoScheduleConfigs<T, Marker>
where
    T: Schedulable<Metadata = GraphInfo, GroupMetadata = Chain>,
{
    fn in_state<S>(self, state: S) -> ScheduleConfigs<T>
    where
        S: States,
    {
        self.into_configs().run_if(in_state(state))
    }
}

impl<S, T, M> ScheduleExt<T, M> for S
where
    S: IntoScheduleConfigs<T, M>,
    T: Schedulable<Metadata = GraphInfo, GroupMetadata = Chain>,
{
}
