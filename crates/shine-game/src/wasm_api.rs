use crate::math::{
    hex::{AxialDenseIndexer, PatchCoord, PatchDenseIndexer, PatchMesher, PatchOrientation},
    rand::Xorshift32,
};
use serde::Deserialize;
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

fn default_method() -> String { "None".to_string() }
fn default_iterations() -> u32 { 20 }
fn default_strength() -> f32 { 0.4 }
fn default_weight_min() -> f32 { 2.5 }
fn default_weight_max() -> f32 { 15.5 }
fn default_amplitude() -> f32 { 0.5 }
fn default_frequency() -> f32 { 5.0 }
fn default_dt() -> f32 { 0.1 }
fn default_spring_strength() -> f32 { 0.3 }
fn default_shape_strength() -> f32 { 0.5 }

#[derive(Deserialize)]
struct FixQuadsConfig {
    #[serde(default = "default_fix_enabled")]
    enabled: bool,
    #[serde(default = "default_min_quality")]
    min_quality: f32,
    #[serde(default = "default_fix_max_iterations")]
    max_iterations: u32,
}

fn default_fix_enabled() -> bool { true }
fn default_min_quality() -> f32 { 0.15 }
fn default_fix_max_iterations() -> u32 { 50 }

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
}

/// Generate a hex quad mesh from a JSON config string.
#[wasm_bindgen]
pub fn generate_mesh(config_json: &str) -> Result<WasmPatchMesh, JsValue> {
    let config: MeshConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

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
            mesher.smooth_noise(
                config.smoothing.amplitude,
                config.smoothing.frequency,
                &mut vertices,
            );
        }
        "Cotangent" => {
            mesher.smooth_cotangent(
                config.smoothing.iterations,
                config.smoothing.strength,
                &mut vertices,
            );
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

    Ok(WasmPatchMesh {
        vertices: flat_vertices,
        indices,
        patch_indices: patch_indices_out,
    })
}
