use crate::map::Tile;
use bevy::ecs::component::Component;
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// Current version of a map-chunk of type T.
#[derive(Component)]
pub struct MapChunkVersion<T>
where
    T: Tile,
{
    pub version: usize,
    ph: PhantomData<T>,
}

impl<T> Default for MapChunkVersion<T>
where
    T: Tile,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> MapChunkVersion<T>
where
    T: Tile,
{
    pub fn new() -> Self {
        Self { version: 0, ph: PhantomData }
    }

    pub fn with_version(version: usize) -> Self {
        Self { version, ph: PhantomData }
    }
}

impl<T> Deref for MapChunkVersion<T>
where
    T: Tile,
{
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.version
    }
}

impl<T> DerefMut for MapChunkVersion<T>
where
    T: Tile,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.version
    }
}
