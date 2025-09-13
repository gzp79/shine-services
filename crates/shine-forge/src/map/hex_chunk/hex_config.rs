use crate::map::Tile;
use bevy::ecs::resource::Resource;
use std::marker::PhantomData;

/// Defines the configuration for a hexagonal chunk, parameterized by a tile type `T`.
#[derive(Resource, Clone)]
pub struct HexConfig<T>
where
    T: Tile,
{
    pub radius: u32,
    _ph: PhantomData<T>,
}

impl<T> HexConfig<T>
where
    T: Tile,
{
    pub fn new(radius: u32) -> Self {
        Self { radius, _ph: PhantomData }
    }
}
