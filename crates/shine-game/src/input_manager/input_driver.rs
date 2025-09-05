use bevy::{ecs::entity::Entity, time::Time, window::Window};
use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
};

pub trait InputDriver: Any + 'static {}

impl InputDriver for Time {}
impl InputDriver for Window {}

pub trait AnyInputDriver {
    fn as_any(&self) -> &dyn Any;
}

impl<T> AnyInputDriver for T
where
    T: InputDriver,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct InputDrivers<'w> {
    pub resources: HashMap<TypeId, &'w dyn Any>,
    pub components: HashMap<(Entity, TypeId), &'w dyn Any>,
    pub markers: HashSet<TypeId>,
}

impl Default for InputDrivers<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'w> InputDrivers<'w> {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            components: HashMap::new(),
            markers: HashSet::new(),
        }
    }

    pub fn add_marker<T>(&mut self)
    where
        T: InputDriver,
    {
        self.markers.insert(TypeId::of::<T>());
    }

    pub fn has_marker<T>(&self) -> bool
    where
        T: InputDriver,
    {
        self.markers.contains(&TypeId::of::<T>())
    }

    pub fn add_resource<T>(&mut self, source: &'w T)
    where
        T: InputDriver,
    {
        self.resources.insert(source.type_id(), source.as_any());
    }

    pub fn get_resource<T>(&self) -> Option<&T>
    where
        T: InputDriver,
    {
        self.resources
            .get(&TypeId::of::<T>())
            .and_then(|s| s.downcast_ref::<T>())
    }

    pub fn add_component<T>(&mut self, entity: Entity, source: &'w T)
    where
        T: InputDriver,
    {
        self.components.insert((entity, source.type_id()), source.as_any());
    }

    pub fn get_all_components<T>(&self) -> impl Iterator<Item = &T>
    where
        T: InputDriver,
    {
        self.components
            .iter()
            .filter_map(|(_, &source)| source.downcast_ref::<T>())
    }

    pub fn get_component<T>(&self, entity: Entity) -> Option<&T>
    where
        T: InputDriver,
    {
        self.components
            .get(&(entity, TypeId::of::<T>()))
            .and_then(|source| source.downcast_ref::<T>())
    }
}
