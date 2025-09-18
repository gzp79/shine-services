use crate::map::{MapLayerConfig, Tile};
use bevy::ecs::resource::Resource;
use std::marker::PhantomData;

/// The configuration for a hexagonal layer
#[derive(Resource, Clone)]
pub struct HexLayerConfig<T>
where
    T: Tile,
{
    pub radius: u32,
    _ph: PhantomData<T>,
}

impl<T> HexLayerConfig<T>
where
    T: Tile,
{
    pub fn new(radius: u32) -> Self {
        Self { radius, _ph: PhantomData }
    }
}

impl<T> MapLayerConfig for HexLayerConfig<T> where T: Tile {}
