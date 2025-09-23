use crate::map::{MapLayer, MapLayerChecksum, MapLayerVersion};
use bevy::ecs::component::Component;
use std::marker::PhantomData;

/// Some meta information of a layer for change tracking.
#[derive(Component)]
pub struct MapLayerInfo<L>
where
    L: MapLayer,
{
    pub version: MapLayerVersion,
    pub checksum: MapLayerChecksum,
    ph: PhantomData<L>,
}

impl<L> Default for MapLayerInfo<L>
where
    L: MapLayer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<L> MapLayerInfo<L>
where
    L: MapLayer,
{
    pub fn new() -> Self {
        Self {
            version: MapLayerVersion::new(),
            checksum: MapLayerChecksum::new(),
            ph: PhantomData,
        }
    }
}
