use crate::math::{
    cdt::Triangulation,
    hex::{CdtMesher, LatticeMesher, PatchMesher, PatchOrientation},
    mesh::{EnergyRelax, Jitter, LaplacianSmoother, QuadFilter, QuadMesh, QuadRelax, VertexRepulsion},
    rand::{StableRng, Xorshift32},
};
use glam::IVec2;
use serde::Deserialize;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

// ── Config types ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct MeshConfig {
    mesher: MesherConfig,
    seed: u32,
    #[serde(default)]
    filters: Vec<FilterConfig>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum MesherConfig {
    Patch {
        subdivision: u32,
        orientation: String,
    },
    Cdt {
        edge_subdivisions: u32,
        interior_points: u32,
    },
    Lattice {
        subdivision: u32,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum FilterConfig {
    Laplacian {
        iterations: u32,
        strength: f32,
    },
    Jitter {
        amplitude: f32,
    },
    QuadRelax {
        min_quality: f32,
        strength: f32,
        max_iterations: u32,
    },
    EnergyRelax {
        area_weight: f32,
        shape_weight: f32,
        step_size: f32,
        iterations: u32,
    },
    VertexRepulsion {
        strength: f32,
        iterations: u32,
    },
}

// ── WASM output ───────────────────────────────────────────────────────

#[wasm_bindgen]
pub struct WasmPatchMesh {
    vertices: Vec<f32>,
    indices: Vec<u32>,
    patch_indices: Vec<u8>,
    dual_vertices: Vec<f32>,
    dual_indices: Vec<u32>,
}

#[wasm_bindgen]
impl WasmPatchMesh {
    /// Flat vertex positions [x, y, x, y, ...] (2 floats per vertex)
    pub fn vertices(&self) -> Vec<f32> {
        self.vertices.clone()
    }

    /// Flat quad indices [a, b, c, d, ...] (4 indices per quad)
    pub fn indices(&self) -> Vec<u32> {
        self.indices.clone()
    }

    /// Patch index per quad (0 for all currently)
    pub fn patch_indices(&self) -> Vec<u8> {
        self.patch_indices.clone()
    }

    /// Number of vertices
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / 2
    }

    /// Number of quads
    pub fn quad_count(&self) -> usize {
        self.indices.len() / 4
    }

    /// Flat dual vertex positions [x, y, x, y, ...] (2 floats per vertex, one per primal quad centroid)
    pub fn dual_vertices(&self) -> Vec<f32> {
        self.dual_vertices.clone()
    }

    /// Flat dual edge indices [a, b, ...] (2 indices per dual edge)
    pub fn dual_indices(&self) -> Vec<u32> {
        self.dual_indices.clone()
    }

    /// Number of dual vertices
    pub fn dual_vertex_count(&self) -> usize {
        self.dual_vertices.len() / 2
    }

    /// Number of dual edges
    pub fn dual_edge_count(&self) -> usize {
        self.dual_indices.len() / 2
    }
}

/// Generate a hex quad mesh from a JSON config string.
///
/// Config shape:
/// ```json
/// {
///   "mesher": { "type": "Patch", "subdivision": 3, "orientation": "Even" }
///           | { "type": "Cdt", "edge_subdivisions": 4, "interior_points": 20 },
///   "seed": 42,
///   "filters": [
///     { "type": "Jitter", "amplitude": 0.3 },
///     { "type": "Laplacian", "iterations": 20, "strength": 0.5 },
///     { "type": "QuadRelax", "min_quality": 0.15, "strength": 0.5, "max_iterations": 50 }
///   ]
/// }
/// ```
#[wasm_bindgen]
pub fn generate_mesh(config_json: &str) -> Result<WasmPatchMesh, JsValue> {
    let config: MeshConfig = serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    // Validate
    match &config.mesher {
        MesherConfig::Patch { subdivision, .. } => {
            if *subdivision > 5 {
                return Err(JsValue::from_str("subdivision must be 0-5"));
            }
        }
        MesherConfig::Cdt { edge_subdivisions, interior_points } => {
            if *edge_subdivisions > 5 {
                return Err(JsValue::from_str("edge_subdivisions must be 1-5"));
            }
            if *interior_points > 500 {
                return Err(JsValue::from_str("interior_points must be 0-500"));
            }
        }
        MesherConfig::Lattice { subdivision } => {
            if *subdivision > 5 {
                return Err(JsValue::from_str("subdivision must be 0-5"));
            }
        }
    }

    // Step 1: Generate base mesh from selected mesher
    let mut mesh = match config.mesher {
        MesherConfig::Patch { subdivision, orientation } => {
            let orient = match orientation.as_str() {
                "Even" => PatchOrientation::Even,
                "Odd" => PatchOrientation::Odd,
                _ => return Err(JsValue::from_str("orientation must be 'Even' or 'Odd'")),
            };
            let mut mesher = PatchMesher::new(subdivision, orient).with_world_size(1.0);
            mesher.generate_uniform()
        }
        MesherConfig::Cdt {
            edge_subdivisions,
            interior_points,
        } => {
            let rng = Xorshift32::new(config.seed);
            let mut mesher = CdtMesher::new(edge_subdivisions, interior_points, rng).with_world_size(1.0);
            mesher.generate()
        }
        MesherConfig::Lattice { subdivision } => {
            let rng = Xorshift32::new(config.seed);
            let mut mesher = LatticeMesher::new(subdivision, rng).with_world_size(1.0);
            mesher.generate()
        }
    };

    // Step 2: Apply filter pipeline
    let mut rng_counter = 0u32;
    for filter_cfg in config.filters {
        let mut filter: Box<dyn QuadFilter> = match filter_cfg {
            FilterConfig::Laplacian { iterations, strength } => {
                Box::new(LaplacianSmoother::new(strength, iterations))
            }
            FilterConfig::Jitter { amplitude } => {
                let rng = Xorshift32::new(config.seed.wrapping_add(rng_counter));
                rng_counter = rng_counter.wrapping_add(1);
                Box::new(Jitter::new(amplitude, rng))
            }
            FilterConfig::QuadRelax {
                min_quality,
                strength,
                max_iterations,
            } => Box::new(QuadRelax::new(min_quality, strength, max_iterations)),
            FilterConfig::EnergyRelax {
                area_weight,
                shape_weight,
                step_size,
                iterations,
            } => Box::new(EnergyRelax::new(area_weight, shape_weight, step_size, iterations)),
            FilterConfig::VertexRepulsion {
                strength,
                iterations,
            } => Box::new(VertexRepulsion::new(strength, iterations)),
        };
        filter.apply(&mut mesh);
    }

    // Step 3: Convert QuadMesh to flat buffers
    Ok(quad_mesh_to_wasm(&mesh))
}

/// Convert a QuadMesh into the flat-buffer WasmPatchMesh format.
fn quad_mesh_to_wasm(mesh: &QuadMesh) -> WasmPatchMesh {
    use crate::indexed::TypedIndex;

    let vertex_count = mesh.vertex_count();
    let quad_count = mesh.quad_count();

    // Flatten positions
    let mut flat_vertices = Vec::with_capacity(vertex_count * 2);
    for vi in mesh.vertex_indices() {
        let p = mesh.position(vi);
        flat_vertices.push(p.x);
        flat_vertices.push(p.y);
    }

    // Flatten quad indices
    let mut indices = Vec::with_capacity(quad_count * 4);
    let mut patch_indices = Vec::with_capacity(quad_count);
    for qi in mesh.quad_indices() {
        let verts = mesh.quad_vertices(qi);
        for &v in &verts {
            indices.push(v.into_index() as u32);
        }
        patch_indices.push(0u8);
    }

    // Compute dual mesh
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

    WasmPatchMesh {
        vertices: flat_vertices,
        indices,
        patch_indices,
        dual_vertices,
        dual_indices,
    }
}

// ── CDT visualization (unchanged) ────────────────────────────────────

#[wasm_bindgen]
pub struct WasmCdt {
    vertices: Vec<f32>,
    triangles: Vec<u32>,
    fixed_edges: Vec<u32>,
    error: Option<String>,
}

#[wasm_bindgen]
impl WasmCdt {
    pub fn vertices(&self) -> Vec<f32> {
        self.vertices.clone()
    }
    pub fn triangles(&self) -> Vec<u32> {
        self.triangles.clone()
    }
    pub fn fixed_edges(&self) -> Vec<u32> {
        self.fixed_edges.clone()
    }
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / 2
    }
    pub fn triangle_count(&self) -> usize {
        self.triangles.len() / 3
    }
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }
    pub fn error_message(&self) -> Option<String> {
        self.error.clone()
    }
}

/// Generate a CDT from random points and constraint edges.
/// `config_json`: { "n_points": u32, "n_edges": u32, "seed": u32, "bound": i32 }
#[wasm_bindgen]
pub fn generate_cdt(config_json: &str) -> WasmCdt {
    #[derive(Deserialize)]
    struct CdtConfig {
        n_points: u32,
        n_edges: u32,
        seed: u32,
        #[serde(default = "default_bound")]
        bound: i32,
    }
    fn default_bound() -> i32 {
        4096
    }

    let config: CdtConfig = match serde_json::from_str(config_json) {
        Ok(c) => c,
        Err(e) => {
            return WasmCdt {
                vertices: vec![],
                triangles: vec![],
                fixed_edges: vec![],
                error: Some(e.to_string()),
            };
        }
    };

    let mut rng = Xorshift32::new(config.seed);
    let bound = config.bound;
    let n = config.n_points.max(3) as usize;

    let range = (bound * 2) as u32;
    let mut points: Vec<IVec2> = Vec::with_capacity(n);
    for _ in 0..n {
        let x = (rng.next_u32() % range) as i32 - bound;
        let y = (rng.next_u32() % range) as i32 - bound;
        points.push(IVec2::new(x, y));
    }

    let m = config.n_edges as usize;
    let mut edges: Vec<(usize, usize)> = Vec::with_capacity(m);
    for _ in 0..m {
        let a = (rng.next_u32() as usize) % n;
        let mut b = (rng.next_u32() as usize) % n;
        if b == a {
            b = (a + 1) % n;
        }
        edges.push((a, b));
    }

    let vertices: Vec<f32> = points.iter().flat_map(|p| [p.x as f32, p.y as f32]).collect();

    let result = if edges.is_empty() {
        Triangulation::build(&points)
    } else {
        Triangulation::build_with_edges_skip_crossing(&points, &edges)
    };

    match result {
        Ok(t) => {
            let triangles: Vec<u32> = t
                .triangles()
                .flat_map(|(a, b, c)| [a as u32, b as u32, c as u32])
                .collect();
            let fixed_edges: Vec<u32> = edges.iter().flat_map(|&(a, b)| [a as u32, b as u32]).collect();

            WasmCdt {
                vertices,
                triangles,
                fixed_edges,
                error: None,
            }
        }
        Err(e) => WasmCdt {
            vertices,
            triangles: vec![],
            fixed_edges: vec![],
            error: Some(e.to_string()),
        },
    }
}
