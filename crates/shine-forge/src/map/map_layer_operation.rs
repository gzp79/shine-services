use crate::map::MapAuditedLayer;
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

pub trait MapLayerOperation<L>: fmt::Debug + Send + Sync + 'static
where
    L: MapAuditedLayer,
{
    fn name(&self) -> &str;
    fn apply(&self, chunk: &mut L, audit: Option<&mut L::Audit>) -> MapLayerChecksum;
}

#[allow(type_alias_bounds)]
pub type BoxedMapLayerOperation<L: MapAuditedLayer> = SmallBox<dyn MapLayerOperation<L>, smallbox::space::S32>;

pub trait MapChunkOperationExt<L>: MapLayerOperation<L>
where
    L: MapAuditedLayer,
{
    fn boxed(self) -> BoxedMapLayerOperation<L>
    where
        Self: Sized + 'static,
    {
        smallbox!(self)
    }
}
impl<T, L> MapChunkOperationExt<L> for T
where
    T: MapLayerOperation<L>,
    L: MapAuditedLayer,
{
}
