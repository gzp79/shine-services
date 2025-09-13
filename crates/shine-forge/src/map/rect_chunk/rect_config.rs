use crate::map::Tile;
use bevy::ecs::resource::Resource;
use std::marker::PhantomData;

/// Defines the configuration for a rectangular chunk, parameterized by a tile type `T`.
#[derive(Resource, Clone)]
pub struct RectConfig<T>
where
    T: Tile,
{
    pub width: u32,
    pub height: u32,
    _ph: PhantomData<T>,
}

impl<T> RectConfig<T>
where
    T: Tile,
{
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            _ph: PhantomData,
        }
    }
}
