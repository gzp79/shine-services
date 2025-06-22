use std::any::Any;

use crate::bevy_utils::input_manager::{ActionLike, InputMap};
use bevy::{
    ecs::{
        resource::Resource,
        system::{Res, ResMut, StaticSystemParam, SystemParam},
    },
    time::Time,
};

pub trait InputSource: Resource + Send + Sync + 'static {
    //fn integrate<A: ActionLike>(provider: Res<Self>, time: Res<Time>, input_map: &mut InputMap<A>);
}

pub trait AnyInputSource: InputSource {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T> AnyInputSource for T
where
    T: InputSource + Any + Sync + Send,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub trait InputProvider: InputSource {
    fn integrate<A: ActionLike>(provider: Res<Self>, time: Res<Time>, input_map: &mut InputMap<A>);
}

pub trait ComputedInputProvider: InputSource {
    type SourceData: SystemParam;

    fn compute(source: StaticSystemParam<Self::SourceData>, provider: ResMut<Self>);
}
