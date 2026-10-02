use crate::{
    indexed::{BitSet, TypedIndex},
    math::quadrangulation::Rot4Idx,
    world::{CellIndex, Chunk, TileIndex},
};
use std::ops::Index;

/// Selects one of a chunk's layers, exposing its component type and storage slot.
pub trait LayerKind {
    type Component: Clone + 'static;

    fn field(chunk: &mut Chunk) -> &mut Option<Layer<Self::Component>>;
}

/// A mutation applied to the layer selected by its `Kind` while the layer is locked.
pub trait LayerUpdate {
    type Kind: LayerKind;

    fn update(&self, builder: &mut LayerBuilder<'_, <Self::Kind as LayerKind>::Component>);
}

/// A layer kind is itself a no-op update, locking its layer for a read-only sync.
impl<K: LayerKind> LayerUpdate for K {
    type Kind = K;

    fn update(&self, _builder: &mut LayerBuilder<'_, K::Component>) {}
}

/// A container type that packs one `Component` per tile cell (`Rot4Idx`-indexed dual vertex).
pub trait Tile {
    type Component;

    fn get(&self, cell: Rot4Idx) -> Self::Component;

    /// Updates the value of the specified cell and returns `true` if the value changed.
    fn set(&mut self, cell: Rot4Idx, value: Self::Component) -> bool;

    /// Builds a tile from its 4 quadrant values, in `Rot4Idx` order.
    fn from_quadrants(values: [Self::Component; 4]) -> Self;
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

    #[inline]
    fn from_quadrants(values: [u8; 4]) -> Self {
        u32::from_le_bytes(values)
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

    pub fn builder<'a>(&'a mut self, chunk: &'a Chunk) -> LayerBuilder<'a, T> {
        LayerBuilder {
            chunk,
            data: &mut self.data,
            change_log: &mut self.change_log,
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
}

/// Builder for a `Layer`, providing mutable, low level access to its data, change log, and the
/// owning chunk's topology (needed to resolve cells to the tiles/quadrants they touch).
pub struct LayerBuilder<'a, T> {
    chunk: &'a Chunk,
    data: &'a mut [T],
    change_log: &'a mut BitSet,
}

impl<'a, T: Clone> LayerBuilder<'a, T> {
    /// Sets every tile in the layer to `value`.
    pub fn fill(&mut self, value: T) {
        self.data.fill(value);
        self.change_log.fill(true);
    }
}

impl<'a, T: Tile> LayerBuilder<'a, T> {
    /// Sets `value` on `quadrant` of `tile`, recording the change in the log if it differs.
    pub fn set_quadrant(&mut self, tile: TileIndex, quadrant: Rot4Idx, value: T::Component) {
        let idx = tile.into_index();
        if self.data[idx].set(quadrant, value) {
            self.change_log.set(idx, true);
        }
    }

    /// Tiles touching `cell`, paired with the quadrant (tile-local corner) each one sees it at.
    /// Ghost tiles outside this chunk (boundary cells) are skipped.
    pub fn cell_tiles(&self, cell: CellIndex) -> impl Iterator<Item = (TileIndex, Rot4Idx)> + 'a {
        let chunk = self.chunk;
        let vi = chunk.cell_to_vert()[cell];
        chunk.mesh().vertex_ring_ccw(vi).filter_map(move |qv| {
            let tile = chunk.quad_to_tile()[qv.quad];
            (!tile.is_none()).then(|| (tile, qv.local))
        })
    }

    /// Sets `value` on the quadrant of every tile touching `cell`, within this chunk.
    pub fn set_cell(&mut self, cell: CellIndex, value: T::Component)
    where
        T::Component: Clone,
    {
        for (tile, quadrant) in self.cell_tiles(cell) {
            self.set_quadrant(tile, quadrant, value.clone());
        }
    }
}
