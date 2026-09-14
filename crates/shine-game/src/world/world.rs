use crate::{
    indexed::TypedIndex,
    math::{
        hex::{HexFlatDir, HexPointyDir},
        prng::SplitMix64,
        quadrangulation::VertexIndex,
    },
    world::{Chunk, ChunkHandle, ChunkId, CornerCells, EdgeCells, InnerCells},
};
use std::{cell::RefCell, collections::HashMap, rc::{Rc, Weak}};
use tracing::info_span;

/// The core subdivision depth to align chunks
pub const SUBDIVISION_BASE: u32 = 4;
/// The numbe of cells on the edge of a chunk
pub const SUBDIVISION_COUNT: u32 = 2u32.pow(SUBDIVISION_BASE);

/// The world size (circumcenter) of a chunk (in meter)
pub const CHUNK_WORLD_SIZE: f32 = 1000.0;
/// The "ideal" length of the side of a cell (in meter)
pub const CELL_WORLD_SIZE: f32 = CHUNK_WORLD_SIZE / SUBDIVISION_COUNT as f32;

/// The world data.
struct WorldInner {
    rng_seed: SplitMix64,
    chunks: HashMap<ChunkId, Chunk>,
    next_chunk_generation: u64,
}

impl WorldInner {
    fn new() -> Self {
        Self {
            rng_seed: SplitMix64::new(),
            chunks: HashMap::new(),
            next_chunk_generation: 0,
        }
    }

    fn init_chunk(&mut self, id: ChunkId) {
        self.next_chunk_generation += 1;
        let generation = self.next_chunk_generation;
        self.chunks.insert(id, Chunk::new(&self.rng_seed, id, generation));
    }

    fn chunk(&self, id: ChunkId) -> Option<&Chunk> {
        self.chunks.get(&id)
    }

    fn remove_chunk(&mut self, id: ChunkId) {
        self.chunks.remove(&id);
    }

    fn chunk_world_offset(&self, reference: ChunkId, target: ChunkId) -> Vec<f32> {
        if self.chunk(reference).is_none() {
            return vec![];
        }
        let offset = reference.relative_world_position(target);
        vec![offset.x, offset.y]
    }

    fn inner_cells(&self, id: ChunkId) -> Option<InnerCells> {
        let _span = info_span!("internal_cells", id = ?id).entered();
        self.chunk(id).map(|chunk| chunk.cell_data())
    }

    fn edge_cells(&self, id: ChunkId, edge_idx: HexFlatDir) -> Option<EdgeCells> {
        let _span = info_span!("edge_cells", id = ?id).entered();

        let (neighbor_dir, neighbor_edge) = match edge_idx {
            HexFlatDir::NE => (HexFlatDir::NE, HexFlatDir::SW),
            HexFlatDir::N => (HexFlatDir::N, HexFlatDir::S),
            HexFlatDir::NW => (HexFlatDir::NW, HexFlatDir::SE),
            HexFlatDir::SW => (HexFlatDir::SW, HexFlatDir::NE),
            HexFlatDir::S => (HexFlatDir::S, HexFlatDir::N),
            HexFlatDir::SE => (HexFlatDir::SE, HexFlatDir::NW),
        };

        let neighbor_id = id.neighbor(neighbor_dir);
        let owner = self.chunk(id)?;
        let neighbor = self.chunk(neighbor_id)?;
        let neighbor_offset = id.relative_world_position(neighbor_id);

        // Collect neighbor inner vertices reversed: pop both corners, reverse in-place.
        // The neighbor edge runs in the opposite direction to the owner edge, so reversing
        // aligns vertex positions pairwise.
        let neighbor_vis: Vec<VertexIndex> = {
            let mut v: Vec<_> = neighbor.boundary_edge_vertices(neighbor_edge).collect();
            v.pop();
            v.reverse();
            v.pop();
            v
        };
        let owner_vis = owner.boundary_edge_vertices(edge_idx).skip(1).take(neighbor_vis.len());

        let site_count = neighbor_vis.len();

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut ranges = Vec::with_capacity(site_count * 2);
        let mut cell_ids = Vec::with_capacity(site_count * 2);
        let mut tile_ids = Vec::new();
        let mut tile_distortions = Vec::new();
        let mut tile_vertices = Vec::new();

        // map from QuadIndex to index in vertices
        let mut index_map = HashMap::new();
        let mut neighbor_index_map = HashMap::new();

        for (vi_owner, vi_neighbor) in owner_vis.zip(neighbor_vis) {
            ranges.push(indices.len() as u32);
            cell_ids.push(owner.vert_to_cell()[vi_owner].into_index() as u32);
            cell_ids.push(neighbor.vert_to_cell()[vi_neighbor].into_index() as u32);

            for qi in owner.mesh().boundary_dual_vertices(vi_owner) {
                let index = *index_map.entry(qi).or_insert_with(|| {
                    let p = owner.mesh().dual_p(qi).unwrap();
                    let idx = (vertices.len() / 2) as u32;
                    vertices.push(p.x);
                    vertices.push(p.y);
                    tile_ids.push(0); // owner tile id is always 0
                    tile_ids.push(owner.quad_to_tile()[qi].into_index() as u32);
                    for &qv in owner.mesh().quad_vertices(qi) {
                        tile_distortions.push(owner.mesh()[qv].position.x);
                        tile_distortions.push(owner.mesh()[qv].position.y);
                    }
                    idx
                });
                indices.push(index);
                tile_vertices.push(owner.mesh().quad_local_vertex(qi, vi_owner).unwrap().into());
            }
            for qi in neighbor.mesh().boundary_dual_vertices(vi_neighbor) {
                let index = *neighbor_index_map.entry(qi).or_insert_with(|| {
                    let p = neighbor.mesh().dual_p(qi).unwrap() + neighbor_offset;
                    let idx = (vertices.len() / 2) as u32;
                    vertices.push(p.x);
                    vertices.push(p.y);
                    tile_ids.push(1); // neighbor tile id is always 1
                    tile_ids.push(neighbor.quad_to_tile()[qi].into_index() as u32);
                    for &qv in neighbor.mesh().quad_vertices(qi) {
                        tile_distortions.push(neighbor.mesh()[qv].position.x);
                        tile_distortions.push(neighbor.mesh()[qv].position.y);
                    }
                    idx
                });
                indices.push(index);
                tile_vertices.push(neighbor.mesh().quad_local_vertex(qi, vi_neighbor).unwrap().into());
            }
            ranges.push(indices.len() as u32);
        }

        Some(EdgeCells {
            vertices,
            indices,
            ranges,
            cell_ids,
            tile_ids,
            tile_vertices,
            tile_distortions,
        })
    }

    fn corner_cells(&self, id: ChunkId, corner_idx: HexPointyDir) -> Option<CornerCells> {
        let _span = info_span!("corner_cells", id = ?id).entered();

        let v0 = corner_idx;
        let (n1, v1, n2, v2) = match corner_idx {
            HexPointyDir::E => (HexFlatDir::SE, HexPointyDir::NW, HexFlatDir::NE, HexPointyDir::SW),
            HexPointyDir::NE => (HexFlatDir::NE, HexPointyDir::W, HexFlatDir::N, HexPointyDir::SE),
            HexPointyDir::NW => (HexFlatDir::N, HexPointyDir::SW, HexFlatDir::NW, HexPointyDir::E),
            HexPointyDir::W => (HexFlatDir::NW, HexPointyDir::SE, HexFlatDir::SW, HexPointyDir::NE),
            HexPointyDir::SW => (HexFlatDir::SW, HexPointyDir::E, HexFlatDir::S, HexPointyDir::NW),
            HexPointyDir::SE => (HexFlatDir::S, HexPointyDir::NE, HexFlatDir::SE, HexPointyDir::W),
        };

        let id0 = id;
        let id1 = id.neighbor(n1);
        let id2 = id.neighbor(n2);

        let chunk0 = self.chunk(id0)?;
        let chunk1 = self.chunk(id1)?;
        let chunk2 = self.chunk(id2)?;

        let mut vertices = Vec::new();
        let mut cell_ids = Vec::with_capacity(3);
        let mut tile_ids = Vec::new();
        let mut tile_distortions = Vec::new();
        let mut tile_vertices = Vec::new();

        for (cid, id, chunk, corner) in [(0, id0, chunk0, v0), (1, id1, chunk1, v1), (2, id2, chunk2, v2)] {
            let offset = id0.relative_world_position(id);
            let vi = chunk.boundary_corner_vertex(corner);
            cell_ids.push(chunk.vert_to_cell()[vi].into_index() as u32);
            for qi in chunk.mesh().boundary_dual_vertices(vi) {
                let pos = chunk.mesh().dual_p(qi).unwrap() + offset;
                vertices.push(pos.x);
                vertices.push(pos.y);
                tile_ids.push(cid);
                tile_ids.push(chunk.quad_to_tile()[qi].into_index() as u32);
                tile_vertices.push(chunk.mesh().quad_local_vertex(qi, vi).unwrap().into());
                for &qv in chunk.mesh().quad_vertices(qi) {
                    tile_distortions.push(chunk.mesh()[qv].position.x);
                    tile_distortions.push(chunk.mesh()[qv].position.y);
                }
            }
        }

        let vertex_count = (vertices.len() / 2) as u32;
        Some(CornerCells {
            vertices,
            indices: (0..vertex_count).collect(),
            ranges: [0, vertex_count],
            cell_ids,
            tile_ids,
            tile_vertices,
            tile_distortions,
        })
    }
}

/// A cheaply clonable handle to a world. Every clone shares one backing store, so passing a
/// `World` around never copies chunk data; it is the single owning root of the world graph.
#[derive(Clone)]
pub struct World {
    inner: Rc<RefCell<WorldInner>>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(WorldInner::new())),
        }
    }

    pub fn init_chunk(&self, id: ChunkId) {
        self.inner.borrow_mut().init_chunk(id);
    }

    pub fn remove_chunk(&self, id: ChunkId) {
        self.inner.borrow_mut().remove_chunk(id);
    }

    pub fn chunk_world_offset(&self, reference: ChunkId, target: ChunkId) -> Vec<f32> {
        self.inner.borrow().chunk_world_offset(reference, target)
    }

    /// A weak handle to the chunk at `id`, or `None` if none is loaded there. The handle keeps
    /// no strong reference to the world, so holding it never pins chunk memory.
    pub fn chunk(&self, id: ChunkId) -> Option<ChunkHandle> {
        let inner = self.inner.borrow();
        let generation = inner.chunk(id)?.generation();
        Some(ChunkHandle::new(self.downgrade(), id, generation))
    }

    /// Runs `f` with the chunk at `id`, `None` if none is loaded there.
    pub fn with_chunk<R>(&self, id: ChunkId, f: impl FnOnce(&Chunk) -> R) -> Option<R> {
        let inner = self.inner.borrow();
        let chunk = inner.chunk(id)?;
        Some(f(chunk))
    }

    pub fn inner_cells(&self, id: ChunkId) -> Option<InnerCells> {
        self.inner.borrow().inner_cells(id)
    }

    pub fn edge_cells(&self, id: ChunkId, edge_idx: HexFlatDir) -> Option<EdgeCells> {
        self.inner.borrow().edge_cells(id, edge_idx)
    }

    pub fn corner_cells(&self, id: ChunkId, corner_idx: HexPointyDir) -> Option<CornerCells> {
        self.inner.borrow().corner_cells(id, corner_idx)
    }

    pub fn downgrade(&self) -> WeakWorld {
        WeakWorld {
            inner: Rc::downgrade(&self.inner),
        }
    }
}

/// A weak reference to a `World`. `upgrade` returns a `World` handle while the backing store is
/// alive, `None` otherwise.
#[derive(Clone)]
pub struct WeakWorld {
    inner: Weak<RefCell<WorldInner>>,
}

impl WeakWorld {
    pub fn upgrade(&self) -> Option<World> {
        Some(World { inner: self.inner.upgrade()? })
    }
}
