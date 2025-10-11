use crate::map::{MapLayer, RectCoord, RectDenseIndexer, RectLayer, RectLayerConfig, Tile};
use bevy::ecs::component::Component;
use fixedbitset::FixedBitSet;
use std::marker::PhantomData;

/// A 2d rectangular grid of bitset.
/// The T tile generic parameter is used to scope the layer to a specific tile type, but the actual data is just a bitset.
#[derive(Component)]
pub struct RectBitsetLayer<T>
where
    T: Tile,
{
    indexer: RectDenseIndexer,
    data: FixedBitSet,
    _ph: PhantomData<T>,
}

impl<T> RectBitsetLayer<T>
where
    T: Tile,
{
    /// Resets all bits to 0
    pub fn reset_all(&mut self) {
        self.data.clear();
    }

    /// Sets all bits to 1
    pub fn set_all(&mut self) {
        self.data.insert_range(..);
    }

    /// Returns true if the bit at the given coordinate is set (1), false if it is not set (0)
    pub fn get(&mut self, coord: RectCoord) -> bool {
        if self.is_in_bounds(coord) {
            let index = self.indexer.get_dense_index(&coord);
            self.data.contains(index)
        } else {
            panic!("Out of bounds access")
        }
    }

    /// Sets the bit at the given coordinate to 1
    pub fn set(&mut self, coord: RectCoord) {
        if self.is_in_bounds(coord) {
            let index = self.indexer.get_dense_index(&coord);
            self.data.insert(index);
        } else {
            panic!("Out of bounds access")
        }
    }

    /// Returns true if any bit is set (at least one bit is 1)
    pub fn has_any(&self) -> bool {
        self.data.minimum().is_some()
    }

    /// Returns true if no bits are set (all bits are 0)
    pub fn has_none(&self) -> bool {
        self.data.is_clear()
    }

    /// Returns true if all bits are set (all bits are 1)
    pub fn has_all(&self) -> bool {
        //Note: Since FixedBitSet can be larger than the actual size of the layer due to how clearing works
        // it is safer to check the active range only than the entire bitset (the unused bits are always 0)
        self.data.contains_all_in_range(..self.indexer.get_total_size())
    }

    /// Returns an iterator over all coordinates where the bit is set (1)
    pub fn ones(&self) -> impl Iterator<Item = RectCoord> + '_ {
        self.data.ones().map(move |index| self.indexer.get_coord(index))
    }

    /// Returns an iterator over all coordinates where the bit is not set (0)
    pub fn zeroes(&self) -> impl Iterator<Item = RectCoord> + '_ {
        self.data.zeroes().map(move |index| self.indexer.get_coord(index))
    }
}

impl<T> MapLayer for RectBitsetLayer<T>
where
    T: Tile,
{
    type Config = RectLayerConfig<T>;

    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            indexer: RectDenseIndexer::new(0, 0),
            data: FixedBitSet::new(),
            _ph: PhantomData,
        }
    }

    fn clear(&mut self) {
        self.indexer = RectDenseIndexer::new(0, 0);
        //Note: FixedBitSet does not have clear method, but to avoid reallocation we just reset all bits to 0
        // It may be not optimal if the bitset was very large before clear, but configuration of a tile layer
        // usually does not change, hence the size of the bitset should remain the same.
        self.data.clear();
    }

    fn initialize(&mut self, config: &Self::Config) {
        let (width, height) = (config.width, config.height);

        self.indexer = RectDenseIndexer::new(width, height);
        let total_size = self.indexer.get_total_size();

        self.data.grow(total_size);
    }

    fn is_empty(&self) -> bool {
        self.indexer.width() == 0 && self.indexer.height() == 0
    }
}

impl<T> RectLayer for RectBitsetLayer<T>
where
    T: Tile,
{
    fn width(&self) -> u32 {
        self.indexer.width()
    }

    fn height(&self) -> u32 {
        self.indexer.height()
    }
}
