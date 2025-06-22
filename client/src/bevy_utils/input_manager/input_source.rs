use crate::bevy_utils::input_manager::{ActionLike, InputMap};
use bevy::{
    ecs::{
        resource::Resource,
        system::{Res, ResMut},
    },
    input::{keyboard::KeyCode, ButtonInput},
    time::Time,
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

pub trait InputSource: Resource + Any + 'static {}

impl InputSource for Time {}
impl InputSource for ButtonInput<KeyCode> {}

pub trait AnyInputSource {
    fn as_any(&self) -> &dyn Any;
}

impl<T> AnyInputSource for T
where
    T: InputSource,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct InputSources<'w> {
    pub sources: HashMap<TypeId, &'w dyn Any>,
}

impl<'w> InputSources<'w> {
    pub fn new() -> Self {
        Self { sources: HashMap::new() }
    }

    pub fn add_source<T>(&mut self, source: &'w T)
    where
        T: InputSource,
    {
        self.sources.insert(source.type_id(), source.as_any());
    }

    pub fn get_source<T>(&self) -> &T
    where
        T: InputSource,
    {
        self.sources
            .get(&TypeId::of::<T>())
            .and_then(|s| s.downcast_ref::<T>())
            .unwrap_or_else(|| panic!("Source {} not found", std::any::type_name::<T>()))
    }
}

pub fn integrate_default_inputs<A>(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut input_map: ResMut<InputMap<A>>,
) where
    A: ActionLike,
{
    let mut input_source = InputSources::new();
    input_source.add_source(&*time);
    input_source.add_source(&*keyboard);

    input_map.integrate(input_source);
}
