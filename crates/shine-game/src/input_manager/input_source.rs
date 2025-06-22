use crate::input_manager::{ActionLike, InputMap};
use bevy::{
    ecs::{
        entity::Entity,
        system::{Query, Res},
    },
    input::{gamepad::Gamepad, keyboard::KeyCode, ButtonInput},
    time::Time,
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

pub trait InputSource: Any + 'static {}

impl InputSource for Time {}

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
    pub resources: HashMap<TypeId, &'w dyn Any>,
    pub components: HashMap<(Entity, TypeId), &'w dyn Any>,
}

impl<'w> InputSources<'w> {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            components: HashMap::new(),
        }
    }

    pub fn add_resource<T>(&mut self, source: &'w T)
    where
        T: InputSource,
    {
        self.resources.insert(source.type_id(), source.as_any());
    }

    pub fn get_resource<T>(&self) -> Option<&T>
    where
        T: InputSource,
    {
        self.resources
            .get(&TypeId::of::<T>())
            .and_then(|s| s.downcast_ref::<T>())
    }

    pub fn add_component<T>(&mut self, entity: Entity, source: &'w T)
    where
        T: InputSource,
    {
        self.components.insert((entity, source.type_id()), source.as_any());
    }

    pub fn get_component<T>(&self, entity: Entity) -> Option<&T>
    where
        T: InputSource,
    {
        self.components
            .get(&(entity, TypeId::of::<T>()))
            .and_then(|s| s.downcast_ref::<T>())
    }
}

pub fn integrate_default_inputs<A>(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut input_query: Query<&mut InputMap<A>>,
) where
    A: ActionLike,
{
    for mut input_map in input_query.iter_mut() {
        let mut input_source = InputSources::new();
        input_source.add_resource(&*time);
        input_source.add_resource(&*keyboard);
        for (entity, gamepad) in gamepads.iter() {
            input_source.add_component(entity, gamepad);
        }

        input_map.integrate(input_source);
    }
}
