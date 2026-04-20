use crate::{
    indexed::TypedIndex,
    math::{
        hex::{CdtMesher, LatticeMesher, PatchMesher, PatchOrientation},
        mesh::{Jitter, LaplacianSmoother, QuadFilter, QuadMesh, QuadRelax, VertexRepulsion},
        quadrangulation::AnchorIdx,
        rand::Xorshift32,
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
    indices: Vec<u32>,
    patch_indices: Vec<u8>,
    dual_vertices: Vec<f32>,
    dual_indices: Vec<u32>,
    anchor_indices: Vec<u32>,
    anchor_edge_starts: Vec<u32>,
}

#[wasm_bindgen]
impl WasmPatchMesh {
    /// The world size used to generate this mesh
    pub fn world_size(&self) -> f32 {
        self.world_size
    }

    /// Flat vertex positions [x, y, x, y, ...] (2 floats per vertex)
    pub fn vertices(&self) -> Vec<f32> {
        self.vertices.clone()
    }

    /// Number of vertices
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / 2
    }

    /// Flat quad indices [a, b, c, d, ...] (4 indices per quad)
    pub fn quad_indices(&self) -> Vec<u32> {
        self.indices.clone()
    }

    /// Number of quads
    pub fn quad_count(&self) -> usize {
        self.indices.len() / 4
    }

    /// Patch index per quad (0 for all currently)
    pub fn patch_indices(&self) -> Vec<u8> {
        self.patch_indices.clone()
    }

    /// Flat dual vertex positions [x, y, x, y, ...] (2 floats per vertex, one per primal quad centroid)
    pub fn dual_vertices(&self) -> Vec<f32> {
        self.dual_vertices.clone()
    }

    /// Number of dual vertices
    pub fn dual_vertex_count(&self) -> usize {
        self.dual_vertices.len() / 2
    }

    /// Flat dual edge indices [a, b, ...] (2 indices per dual edge)
    pub fn dual_indices(&self) -> Vec<u32> {
        self.dual_indices.clone()
    }

    /// Number of dual edges
    pub fn dual_edge_count(&self) -> usize {
        self.dual_indices.len() / 2
    }

    /// Flat anchor edge indices [a, b, ...] (2 indices per segment)
    pub fn anchor_indices(&self) -> Vec<u32> {
        self.anchor_indices.clone()
    }

    /// Start index in anchor_indices for each anchor edge
    /// Format: [start0, start1, ..., total_segments]
    pub fn anchor_edge_starts(&self) -> Vec<u32> {
        self.anchor_edge_starts.clone()
    }

    /// Number of anchor edges
    pub fn anchor_edge_count(&self) -> usize {
        if self.anchor_edge_starts.is_empty() {
            0
        } else {
            self.anchor_edge_starts.len() - 1
        }
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
            let rng = Xorshift32::new(config.seed);
            let mut mesher = CdtMesher::new(subdivision, interior_points, rng).with_world_size(world_size);
            mesher.generate()
        }
        MesherConfig::Lattice { subdivision } => {
            let rng = Xorshift32::new(config.seed);
            let mut mesher = LatticeMesher::new(subdivision, rng).with_world_size(world_size);
            mesher.generate()
        }
    };

    mesh.topology
        .validate()
        .map_err(|e| JsValue::from_str(&format!("Invalid mesh topology: {:?}", e)))?;

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

fn quad_mesh_to_wasm(mesh: &QuadMesh, world_size: f32) -> WasmPatchMesh {
    let topology = &mesh.topology;
    let vertex_count = topology.finite_vertex_count();
    let quad_count = topology.finite_quad_count();

    let mut flat_vertices = Vec::with_capacity(vertex_count * 2);
    for vi in topology.vertex_indices() {
        let p = mesh.vertices[vi];
        flat_vertices.push(p.x);
        flat_vertices.push(p.y);
    }

    let mut indices = Vec::with_capacity(quad_count * 4);
    let mut patch_indices = Vec::with_capacity(quad_count);
    for qi in topology.quad_indices() {
        let verts = topology.quad_vertices(qi);
        for &v in &verts {
            indices.push(v.into_index() as u32);
        }
        patch_indices.push(0u8);
    }

    let mut dual_vertices = Vec::with_capacity(quad_count * 2);
    for qi_idx in 0..quad_count {
        let base = qi_idx * 4;
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        for k in 0..4 {
            let vi = indices[base + k] as usize;
            cx += flat_vertices[vi * 2];
            cy += flat_vertices[vi * 2 + 1];
        }
        dual_vertices.push(cx / 4.0);
        dual_vertices.push(cy / 4.0);
    }

    let mut edge_map: HashMap<(u32, u32), u32> = HashMap::new();
    let mut dual_indices = Vec::new();
    for qi_idx in 0..quad_count {
        let base = qi_idx * 4;
        for k in 0..4 {
            let a = indices[base + k];
            let b = indices[base + (k + 1) % 4];
            let edge_key = if a < b { (a, b) } else { (b, a) };
            if let Some(&other_qi) = edge_map.get(&edge_key) {
                dual_indices.push(other_qi);
                dual_indices.push(qi_idx as u32);
            } else {
                edge_map.insert(edge_key, qi_idx as u32);
            }
        }
    }

    // Build anchor edges from anchor_vertices in topology
    let mut anchor_indices = Vec::new();
    let mut anchor_edge_starts = Vec::new();
    let anchor_count = topology.anchor_vertices.len();

    for edge_idx in 0..anchor_count {
        // Record the start index for this anchor edge
        anchor_edge_starts.push((anchor_indices.len() / 2) as u32);

        let anchor_verts: Vec<_> = topology.anchor_edge(AnchorIdx::new(edge_idx)).collect();
        // Create line segments between consecutive vertices
        for window in anchor_verts.windows(2) {
            anchor_indices.push(window[0].into_index() as u32);
            anchor_indices.push(window[1].into_index() as u32);
        }
    }

    // Add final count for convenience
    anchor_edge_starts.push((anchor_indices.len() / 2) as u32);

    WasmPatchMesh {
        world_size,
        vertices: flat_vertices,
        indices,
        patch_indices,
        dual_vertices,
        dual_indices,
        anchor_indices,
        anchor_edge_starts,
    }
}
