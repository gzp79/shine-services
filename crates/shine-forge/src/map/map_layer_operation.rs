use crate::map::MapLayer;
use smallbox::{smallbox, SmallBox};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapLayerVersion(pub usize);

impl Default for MapLayerVersion {
    fn default() -> Self {
        Self::new()
    }
}

impl MapLayerVersion {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapLayerChecksum(pub usize);

impl Default for MapLayerChecksum {
    fn default() -> Self {
        Self::new()
    }
}

impl MapLayerChecksum {
    pub fn new() -> Self {
        Self(0)
    }
}

pub trait MapLayerOperation<C>: fmt::Debug + Send + Sync + 'static
where
    C: MapLayer,
{
    fn name(&self) -> &str;
    fn apply(&self, chunk: &mut C) -> MapLayerChecksum;
}

#[allow(type_alias_bounds)]
pub type BoxedMapLayerOperation<C: MapLayer> = SmallBox<dyn MapLayerOperation<C>, smallbox::space::S32>;

pub trait MapChunkOperationExt<C>: MapLayerOperation<C>
where
    C: MapLayer,
{
    fn boxed(self) -> BoxedMapLayerOperation<C>
    where
        Self: Sized + 'static,
    {
        smallbox!(self)
    }
}
impl<T, C> MapChunkOperationExt<C> for T
where
    T: MapLayerOperation<C>,
    C: MapLayer,
{
}
