use crate::{
    indexed::{BitSet, TypedIndex},
    math::quadrangulation::Rot4Idx,
    world::{Chunk, TileIndex},
};
use std::ops::Index;

/// Selects one of a chunk's layers, exposing its component type and storage slot.
pub trait LayerKind {
    type Component: 'static;

    fn field(chunk: &mut Chunk) -> &mut Option<Layer<Self::Component>>;
}

/// A mutation applied to the layer selected by its `Kind` while the layer is locked.
pub trait LayerUpdate {
    type Kind: LayerKind;

    fn update(&self, layer: &mut Layer<<Self::Kind as LayerKind>::Component>);
}

/// A layer kind is itself a no-op update, locking its layer for a read-only sync.
impl<K: LayerKind> LayerUpdate for K {
    type Kind = K;

    fn update(&self, _layer: &mut Layer<K::Component>) {}
}

/// A container type that packs one `Component` per tile cell (`Rot4Idx`-indexed dual vertex).
pub trait Tile {
    type Component;

    fn get(&self, cell: Rot4Idx) -> Self::Component;

    /// Updates the value of the specified cell and returns `true` if the value changed.
    fn set(&mut self, cell: Rot4Idx, value: Self::Component) -> bool;
}

impl Tile for u32 {
    type Component = u8;

    #[inline]
    fn get(&self, cell: Rot4Idx) -> u8 {
        self.to_le_bytes()[cell.into_index()]
    }

    #[inline]
    fn set(&mut self, cell: Rot4Idx, value: u8) -> bool {
        let mut bytes = self.to_le_bytes();
        let idx = cell.into_index();
        if bytes[idx] == value {
            return false;
        }
        bytes[idx] = value;
        *self = u32::from_le_bytes(bytes);
        true
    }
}

/// Per-tile data for a chunk, indexed by `TileIndex`.
pub struct Layer<T> {
    data: Box<[T]>,
    change_log: BitSet,
}

impl<T: Clone> Layer<T> {
    pub fn new(tile_count: usize, value: T) -> Self {
        Self {
            data: vec![value; tile_count].into_boxed_slice(),
            change_log: BitSet::new(tile_count),
        }
    }
}

impl<T> Layer<T> {
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    pub fn log(&self) -> &BitSet {
        &self.change_log
    }

    pub fn clear_log(&mut self) {
        self.change_log.clear();
    }
}

impl<T> Index<TileIndex> for Layer<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: TileIndex) -> &T {
        &self.data[index.into_index()]
    }
}

impl<T: Tile> Layer<T> {
    #[inline]
    pub fn get_tile(&self, tile: TileIndex, cell: Rot4Idx) -> T::Component {
        self[tile].get(cell)
    }

    #[inline]
    pub fn set_tile(&mut self, tile: TileIndex, cell: Rot4Idx, value: T::Component) {
        if self.data[tile.into_index()].set(cell, value) {
            self.change_log.set(tile.into_index(), true);
        }
    }
}
