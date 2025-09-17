use crate::map::{MapLayer, MapLayerChecksum, MapLayerVersion};
use bevy::ecs::component::Component;
use std::marker::PhantomData;

/// Some meta information of a layer for change tracking.
#[derive(Component)]
pub struct MapLayerInfo<C>
where
    C: MapLayer,
{
    pub version: MapLayerVersion,
    pub checksum: MapLayerChecksum,
    ph: PhantomData<C>,
}

impl<C> Default for MapLayerInfo<C>
where
    C: MapLayer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C> MapLayerInfo<C>
where
    C: MapLayer,
{
    pub fn new() -> Self {
        Self {
            version: MapLayerVersion::new(),
            checksum: MapLayerChecksum::new(),
            ph: PhantomData,
        }
    }
}
