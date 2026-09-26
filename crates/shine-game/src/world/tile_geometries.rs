use crate::world::generation::{Generation, GenerationGuard};

#[derive(Debug, Clone, Default)]
struct TileGeometryData {
    /// Tile distortion in tile-array order, packed as [x, y, ...] where each octet corresponds to a single tile
    tile_distortions: Vec<f32>,
    /// Per-tile-edge blend factor `a` such that `mid = a * start + (1 - a) * end`, packed as 4 floats per tile
    /// (one per edge, same winding as `tile_distortions`' 4 corners).
    tile_edge_blends: Vec<f32>,
}

/// Standalone geometry snapshot of a chunk's tiles guarded by the chunk's generation.
pub struct TileGeometries {
    data: TileGeometryData,
    guard: GenerationGuard,
}

impl TileGeometries {
    pub fn new(tile_distortions: Vec<f32>, tile_edge_blends: Vec<f32>, generation: &Generation) -> Self {
        Self {
            data: TileGeometryData {
                tile_distortions,
                tile_edge_blends,
            },
            guard: GenerationGuard::new(generation),
        }
    }

    /// Whether the source chunk is unchanged; `false` means every accessor returns `None`.
    pub fn is_valid(&self) -> bool {
        self.guard.is_valid()
    }

    /// The captured buffers while the source chunk is unchanged, `None` otherwise.
    fn get(&self) -> Option<&TileGeometryData> {
        self.is_valid().then_some(&self.data)
    }

    pub fn tile_distortions(&self) -> Option<&[f32]> {
        self.get().map(|d| d.tile_distortions.as_slice())
    }

    pub fn tile_edge_blends(&self) -> Option<&[f32]> {
        self.get().map(|d| d.tile_edge_blends.as_slice())
    }

    /// Number of tiles (each tile is an octet: 4 corners × [x, y]).
    pub fn tile_count(&self) -> Option<usize> {
        self.get().map(|d| d.tile_distortions.len() / 8)
    }
}
