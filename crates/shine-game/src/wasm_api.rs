use crate::math::{
    cdt::Triangulation,
    hex::{AxialDenseIndexer, PatchCoord, PatchDenseIndexer, PatchMesher, PatchOrientation},
    rand::{StableRng, Xorshift32},
};
use glam::IVec2;
use serde::Deserialize;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[derive(Deserialize)]
struct MeshConfig {
    subdivision: u32,
    orientation: String,
    seed: u32,
    #[serde(default)]
    smoothing: SmoothingConfig,
    #[serde(default)]
    fix_quads: FixQuadsConfig,
}

#[derive(Deserialize, Default)]
struct SmoothingConfig {
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_iterations")]
    iterations: u32,
    #[serde(default = "default_strength")]
    strength: f32,
    #[serde(default = "default_weight_min")]
    weight_min: f32,
    #[serde(default = "default_weight_max")]
    weight_max: f32,
    #[serde(default = "default_amplitude")]
    amplitude: f32,
    #[serde(default = "default_frequency")]
    frequency: f32,
    #[serde(default = "default_dt")]
    dt: f32,
    #[serde(default = "default_spring_strength")]
    spring_strength: f32,
    #[serde(default = "default_shape_strength")]
    shape_strength: f32,
}

fn default_method() -> String {
    "None".to_string()
}
fn default_iterations() -> u32 {
    20
}
fn default_strength() -> f32 {
    0.4
}
fn default_weight_min() -> f32 {
    2.5
}
fn default_weight_max() -> f32 {
    15.5
}
fn default_amplitude() -> f32 {
    0.5
}
fn default_frequency() -> f32 {
    5.0
}
fn default_dt() -> f32 {
    0.1
}
fn default_spring_strength() -> f32 {
    0.3
}
fn default_shape_strength() -> f32 {
    0.5
}

#[derive(Deserialize)]
struct FixQuadsConfig {
    #[serde(default = "default_fix_enabled")]
    enabled: bool,
    #[serde(default = "default_min_quality")]
    min_quality: f32,
    #[serde(default = "default_fix_max_iterations")]
    max_iterations: u32,
}

fn default_fix_enabled() -> bool {
    true
}
fn default_min_quality() -> f32 {
    0.15
}
fn default_fix_max_iterations() -> u32 {
    50
}

impl Default for FixQuadsConfig {
    fn default() -> Self {
        Self {
            enabled: default_fix_enabled(),
            min_quality: default_min_quality(),
            max_iterations: default_fix_max_iterations(),
        }
    }
}

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

    /// Patch index per quad (0, 1, or 2)
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
#[wasm_bindgen]
pub fn generate_mesh(config_json: &str) -> Result<WasmPatchMesh, JsValue> {
    let config: MeshConfig = serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    if config.subdivision > 5 {
        return Err(JsValue::from_str("subdivision must be 0-5"));
    }

    let orientation = match config.orientation.as_str() {
        "Even" => PatchOrientation::Even,
        "Odd" => PatchOrientation::Odd,
        _ => return Err(JsValue::from_str("orientation must be 'Even' or 'Odd'")),
    };

    let rng = Xorshift32::new(config.seed);
    let mut mesher = PatchMesher::new(config.subdivision, orientation, rng);

    // Generate base vertices
    let mut vertices = mesher.create_vertex_buffer();
    mesher.generate_uniform(&mut vertices);

    // Apply smoothing
    match config.smoothing.method.as_str() {
        "None" => {}
        "Lloyd" => {
            mesher.smooth_weighted_lloyd(
                config.smoothing.iterations,
                config.smoothing.strength,
                (config.smoothing.weight_min, config.smoothing.weight_max),
                &mut vertices,
            );
        }
        "Noise" => {
            mesher.smooth_noise(config.smoothing.amplitude, config.smoothing.frequency, &mut vertices);
        }
        "Cotangent" => {
            mesher.smooth_cotangent(config.smoothing.iterations, config.smoothing.strength, &mut vertices);
        }
        "Spring" => {
            mesher.smooth_spring(
                config.smoothing.iterations,
                config.smoothing.dt,
                config.smoothing.spring_strength,
                config.smoothing.shape_strength,
                &mut vertices,
            );
        }
        "Jitter" => {
            mesher.smooth_jitter(config.smoothing.amplitude, &mut vertices);
        }
        other => {
            return Err(JsValue::from_str(&format!("unknown smoothing method: {other}")));
        }
    }

    // Fix quads
    if config.fix_quads.enabled {
        mesher.fix_quads(
            config.fix_quads.min_quality,
            config.fix_quads.max_iterations,
            &mut vertices,
        );
    }

    // Build index arrays
    let radius = 2u32.pow(config.subdivision);
    let axial_indexer = AxialDenseIndexer::new(radius);
    let patch_indexer = PatchDenseIndexer::new(config.subdivision);
    let grid = 2i32.pow(config.subdivision);

    let mut indices = Vec::with_capacity(patch_indexer.get_total_size() * 4);
    let mut patch_indices_out = Vec::with_capacity(patch_indexer.get_total_size());

    for p in 0..3i32 {
        for u in 0..grid {
            for v in 0..grid {
                let patch = PatchCoord::new(p, u, v);
                let quad = patch.quad_vertices(orientation, config.subdivision);
                for coord in &quad {
                    indices.push(axial_indexer.get_dense_index(coord) as u32);
                }
                patch_indices_out.push(p as u8);
            }
        }
    }

    // Flatten Vec2 positions to [x, y, x, y, ...]
    let flat_vertices: Vec<f32> = vertices.iter().flat_map(|v| [v.x, v.y]).collect();

    // Compute dual mesh: vertices are quad centroids, edges connect adjacent quads
    let quad_count = patch_indexer.get_total_size();
    let mut dual_vertices = Vec::with_capacity(quad_count * 2);
    for qi in 0..quad_count {
        let base = qi * 4;
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

    // Build edge-to-quad map: (min_vi, max_vi) -> first quad index
    let mut edge_map: HashMap<(u32, u32), u32> = HashMap::new();
    let mut dual_indices = Vec::new();
    for qi in 0..quad_count {
        let base = qi * 4;
        for k in 0..4 {
            let a = indices[base + k];
            let b = indices[base + (k + 1) % 4];
            let edge_key = if a < b { (a, b) } else { (b, a) };
            if let Some(&other_qi) = edge_map.get(&edge_key) {
                dual_indices.push(other_qi);
                dual_indices.push(qi as u32);
            } else {
                edge_map.insert(edge_key, qi as u32);
            }
        }
    }

    Ok(WasmPatchMesh {
        vertices: flat_vertices,
        indices,
        patch_indices: patch_indices_out,
        dual_vertices,
        dual_indices,
    })
}

// --- CDT visualization ---

#[wasm_bindgen]
pub struct WasmCdt {
    /// Flat vertex positions [x, y, ...] (2 floats per vertex, in input order)
    vertices: Vec<f32>,
    /// Flat triangle indices [a, b, c, ...] (3 indices per triangle)
    triangles: Vec<u32>,
    /// Flat edge indices for fixed/constraint edges [a, b, ...]
    fixed_edges: Vec<u32>,
    /// Error message if triangulation failed
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

    // Generate random points in [-bound, bound]
    let range = (bound * 2) as u32;
    let mut points: Vec<IVec2> = Vec::with_capacity(n);
    for _ in 0..n {
        let x = (rng.next_u32() % range) as i32 - bound;
        let y = (rng.next_u32() % range) as i32 - bound;
        points.push(IVec2::new(x, y));
    }

    // Generate random constraint edges
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

    // Flatten vertex positions
    let vertices: Vec<f32> = points.iter().flat_map(|p| [p.x as f32, p.y as f32]).collect();

    // Skip crossing constraints and keep all triangles (no flood fill)
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
