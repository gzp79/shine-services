use crate::math::{
    hex::{CdtMesher, LatticeMesher, PatchMesher, PatchOrientation},
    prng::{StableRng, Xorshift32},
    quadrangulation::{
        Jitter, LaplacianSmoother, QuadFilter, QuadIndex, QuadRelax, Quadrangulation, VertexIndex, VertexRepulsion,
    },
};
use serde::Deserialize;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[derive(Deserialize)]
struct MeshConfig {
    mesher: MesherConfig,
    seed: u32,
    #[serde(default = "default_world_size")]
    world_size: f32,
    #[serde(default)]
    filters: Vec<FilterConfig>,
}

fn default_world_size() -> f32 {
    1.0
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum MesherConfig {
    Patch { subdivision: u32, orientation: String },
    Cdt { subdivision: u32, interior_points: u32 },
    Lattice { subdivision: u32 },
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum FilterConfig {
    Laplacian {
        strength: f32,
        iterations: u32,
    },
    Jitter {
        amplitude: f32,
    },
    QuadRelax {
        quality: f32,
        strength: f32,
        iterations: u32,
    },
    VertexRepulsion {
        strength: f32,
        iterations: u32,
    },
}

#[wasm_bindgen]
pub struct WasmPatchMesh {
    world_size: f32,
    vertices: Vec<f32>,
    quad_indices: Vec<u32>,
    anchor_indices: Vec<u32>,
    anchor_edge_starts: Vec<u32>,
    dual_vertices: Vec<f32>,
    dual_indices: Vec<u32>,
    dual_polygon_starts: Vec<u32>,
}

#[wasm_bindgen]
impl WasmPatchMesh {
    pub fn world_size(&self) -> f32 {
        self.world_size
    }

    pub fn vertices(&self) -> Vec<f32> {
        self.vertices.clone()
    }
    pub fn quad_indices(&self) -> Vec<u32> {
        self.quad_indices.clone()
    }
    pub fn anchor_indices(&self) -> Vec<u32> {
        self.anchor_indices.clone()
    }
    pub fn anchor_edge_starts(&self) -> Vec<u32> {
        self.anchor_edge_starts.clone()
    }

    pub fn dual_vertices(&self) -> Vec<f32> {
        self.dual_vertices.clone()
    }
    pub fn dual_indices(&self) -> Vec<u32> {
        self.dual_indices.clone()
    }
    pub fn dual_polygon_starts(&self) -> Vec<u32> {
        self.dual_polygon_starts.clone()
    }
}

/// Generate a hex quad mesh from a JSON config string.
#[wasm_bindgen]
pub fn generate_mesh(config_json: &str) -> Result<WasmPatchMesh, JsValue> {
    let config: MeshConfig = serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let world_size = config.world_size;

    // Step 1: Generate base mesh from selected mesher
    let mut mesh = match config.mesher {
        MesherConfig::Patch { subdivision, orientation } => {
            let orient = match orientation.as_str() {
                "Even" => PatchOrientation::Even,
                "Odd" => PatchOrientation::Odd,
                _ => return Err(JsValue::from_str("orientation must be 'Even' or 'Odd'")),
            };
            let mut mesher = PatchMesher::new(subdivision, orient).with_world_size(world_size);
            mesher.generate_uniform()
        }
        MesherConfig::Cdt { subdivision, interior_points } => {
            let rng = Xorshift32::new(config.seed).into_rc();
            let mut mesher = CdtMesher::new(subdivision, interior_points, rng).with_world_size(world_size);
            mesher.generate()
        }
        MesherConfig::Lattice { subdivision } => {
            let rng = Xorshift32::new(config.seed).into_rc();
            let mut mesher = LatticeMesher::new(subdivision, rng).with_world_size(world_size);
            mesher.generate()
        }
    };

    mesh.validate()
        .map_err(|e| JsValue::from_str(&format!("Invalid mesh: {:?}", e)))?;

    // Step 2: Apply filter pipeline
    for filter_cfg in config.filters {
        let mut filter: Box<dyn QuadFilter> = match filter_cfg {
            FilterConfig::Laplacian { iterations, strength } => Box::new(LaplacianSmoother::new(strength, iterations)),
            FilterConfig::Jitter { amplitude } => {
                let rng = Xorshift32::new(config.seed);
                Box::new(Jitter::new(amplitude, rng))
            }
            FilterConfig::QuadRelax { quality, strength, iterations } => {
                Box::new(QuadRelax::new(quality, strength, iterations))
            }
            FilterConfig::VertexRepulsion { strength, iterations } => {
                Box::new(VertexRepulsion::new(strength, iterations))
            }
        };
        filter.apply(&mut mesh);
    }

    // Step 3: Convert QuadMesh to flat buffers
    Ok(quad_mesh_to_wasm(&mesh, world_size))
}

fn quad_mesh_to_wasm(mesh: &Quadrangulation, world_size: f32) -> WasmPatchMesh {
    let mut vert_map: HashMap<VertexIndex, usize> = HashMap::new();
    let mut dual_vert_map: HashMap<QuadIndex, usize> = HashMap::new();

    let mut vertices = Vec::with_capacity(mesh.vertex_count() * 2);

    for vi in mesh.finite_vertex_index_iter() {
        let p = mesh.p(vi);
        vert_map.insert(vi, vert_map.len());
        vertices.push(p.x);
        vertices.push(p.y);
    }

    let mut quad_indices = Vec::with_capacity(mesh.quad_count() * 4);
    for q in mesh.finite_quad_iter() {
        for v in q.vertices.iter() {
            quad_indices.push(vert_map[v] as u32);
        }
    }

    let mut dual_vertices = Vec::with_capacity(mesh.quad_count() * 2);
    for qi in mesh.finite_quad_index_iter() {
        let p = mesh.dual_p(qi).unwrap();
        dual_vert_map.insert(qi, dual_vert_map.len());
        dual_vertices.push(p.x);
        dual_vertices.push(p.y);
    }

    let mut dual_indices = Vec::new();
    let mut dual_polygon_starts = Vec::new();
    for vi in mesh.finite_vertex_index_iter() {
        dual_polygon_starts.push(dual_indices.len() as u32);
        for qvi in mesh.vertex_ring_ccw(vi) {
            if mesh.is_infinite_quad(qvi.quad) {
                // Skip infinite quads
                continue;
            }
            dual_indices.push(dual_vert_map[&qvi.quad] as u32);
        }
    }

    // Build anchor edges from anchor_vertices in mesh
    let mut anchor_indices = Vec::new();
    let mut anchor_edge_starts = Vec::new();

    for ai in mesh.anchor_index_iter() {
        anchor_edge_starts.push(anchor_indices.len() as u32);
        anchor_indices.extend(mesh.anchor_edge(ai).map(|vi| vert_map[&vi] as u32));
    }

    WasmPatchMesh {
        world_size,
        vertices,
        quad_indices,
        anchor_indices,
        anchor_edge_starts,
        dual_vertices,
        dual_indices,
        dual_polygon_starts,
    }
}
