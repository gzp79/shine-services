use crate::{
    dto::IndexedMesh,
    indexed::TypedIndex,
    math::hex::{HexNeighbor, HexVertex},
    wasm::dto::WasmIndexedMesh,
    world::{Chunk, ChunkId, World},
};
use wasm_bindgen::prelude::*;

/// Center chunk for the experiment (easy to modify)
const CENTER_CHUNK: ChunkId = ChunkId::ORIGIN;

/// Internal storage for geometry grouped by chunk/edge/vertex
struct ChunkGeometry {
    hex_vertices: Vec<f32>,    // 12 floats per chunk (6 vertices * 2 coords)
    interior: WasmIndexedMesh, // Interior dual polygons
}

#[wasm_bindgen]
pub struct WasmWorldNeighbors {
    chunks: Vec<ChunkGeometry>,     // 7 chunks (0 = center, 1-6 = neighbors)
    edges: Vec<WasmIndexedMesh>,    // 6 edges
    vertices: Vec<WasmIndexedMesh>, // 6 vertices
}

#[wasm_bindgen]
impl WasmWorldNeighbors {
    /// Returns 12 floats (6 vertices * 2 coords) for the given chunk
    pub fn chunk_hex_vertices(&self, chunk_idx: u32) -> Vec<f32> {
        if chunk_idx >= 7 {
            return vec![];
        }
        self.chunks[chunk_idx as usize].hex_vertices.clone()
    }

    /// Get interior mesh for the given chunk
    pub fn interior_mesh(&self, chunk_idx: u32) -> Option<WasmIndexedMesh> {
        if chunk_idx >= 7 {
            return None;
        }
        Some(self.chunks[chunk_idx as usize].interior.clone())
    }

    /// Get edge mesh for the given edge
    pub fn edge_mesh(&self, edge_idx: u32) -> Option<WasmIndexedMesh> {
        if edge_idx >= 6 {
            return None;
        }
        Some(self.edges[edge_idx as usize].clone())
    }

    /// Get vertex mesh for the given vertex
    pub fn vertex_mesh(&self, vertex_idx: u32) -> Option<WasmIndexedMesh> {
        if vertex_idx >= 6 {
            return None;
        }
        Some(self.vertices[vertex_idx as usize].clone())
    }
}

/// Extract 6 anchor vertices from chunk mesh, transform by offset, return as flat array
fn extract_hex_vertices(chunk: &Chunk, offset: glam::Vec2) -> Vec<f32> {
    use crate::math::quadrangulation::AnchorIndex;
    let mut vertices = Vec::with_capacity(12);

    for i in 0..6 {
        let vi = chunk.mesh.anchor_vertex(AnchorIndex::new(i));
        let p = chunk.mesh.p(vi) + offset;
        vertices.push(p.x);
        vertices.push(p.y);
    }

    vertices
}

/// Extract boundary edge dual polygons from world
fn extract_edge_geometry(world: &World, center_id: ChunkId, edge_idx: u8) -> IndexedMesh {
    match world.boundary_edge_dual_polygons(center_id, HexNeighbor::from(edge_idx)) {
        Some(()) => IndexedMesh::default(),
        None => IndexedMesh::default(),
    }
}

/// Extract boundary vertex dual polygons from world
fn extract_vertex_geometry(world: &World, center_id: ChunkId, vertex_idx: u8) -> IndexedMesh {
    match world.boundary_vertex_dual_polygon(center_id, HexVertex::from(vertex_idx)) {
        Some(vertices) => IndexedMesh::from_polyline(&vertices),
        None => IndexedMesh::default(),
    }
}

/// Generate world neighbors geometry for visualization
#[wasm_bindgen]
pub fn generate_world_neighbors() -> Result<WasmWorldNeighbors, JsValue> {
    let mut world = World::new();

    let center = CENTER_CHUNK;
    let neighbor_ids: Vec<ChunkId> = (0..6).map(|n| center.neighbor(HexNeighbor::from(n))).collect();

    // Initialize chunks
    world.init_chunk(center);
    for neighbor_id in &neighbor_ids {
        world.init_chunk(*neighbor_id);
    }

    let mut chunks = Vec::with_capacity(7);

    //Process center chunk (index 0)
    {
        let chunk = world
            .chunk(center)
            .ok_or_else(|| JsValue::from_str("Center chunk not found"))?;
        let offset = glam::Vec2::ZERO;
        let hex_vertices = extract_hex_vertices(chunk, offset);
        let interior = chunk.mesh.dual_extractor(offset).build_internal_mesh();
        chunks.push(ChunkGeometry {
            hex_vertices,
            interior: interior.into(),
        });
    }

    // Process 6 neighbor chunks (indices 1-6)
    for neighbor_id in &neighbor_ids {
        let chunk = world
            .chunk(*neighbor_id)
            .ok_or_else(|| JsValue::from_str("Neighbor chunk not found"))?;
        let offset = center.relative_world_position(*neighbor_id);
        let hex_vertices = extract_hex_vertices(chunk, offset);
        let interior = chunk.mesh.dual_extractor(offset).build_internal_mesh();
        chunks.push(ChunkGeometry {
            hex_vertices,
            interior: interior.into(),
        });
    }

    let edges: Vec<WasmIndexedMesh> = (0..6)
        .map(|edge_idx| extract_edge_geometry(&world, center, edge_idx).into())
        .collect();

    let vertices_geo: Vec<WasmIndexedMesh> = (0..6)
        .map(|vertex_idx| extract_vertex_geometry(&world, center, vertex_idx).into())
        .collect();

    Ok(WasmWorldNeighbors {
        chunks,
        edges,
        vertices: vertices_geo,
    })
}
