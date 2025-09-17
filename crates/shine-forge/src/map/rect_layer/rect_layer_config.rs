use crate::map::Tile;
use bevy::ecs::resource::Resource;
use std::marker::PhantomData;

/// The configuration for a rectangular layer
#[derive(Resource, Clone)]
pub struct RectLayerConfig<T>
where
    T: Tile,
{
    pub width: u32,
    pub height: u32,
    _ph: PhantomData<T>,
}

impl<T> RectLayerConfig<T>
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
