use crate::{
    math::{
        hex::{PatchMesher, PatchOrientation},
        prng::{StableRng, Xorshift32},
        quadrangulation::{
            Jitter, LaplacianSmoother, QuadError, QuadFilter, QuadRelax, Quadrangulation, VertexRepulsion,
        },
    },
    wasm::dto::WasmIndexedMesh,
};
use glam::Vec2;
use serde::Deserialize;
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
    //Cdt { subdivision: u32, interior_points: u32 },
    //Lattice { subdivision: u32 },
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
    primal: WasmIndexedMesh,
    dual: WasmIndexedMesh,
}

#[wasm_bindgen]
impl WasmPatchMesh {
    pub fn world_size(&self) -> f32 {
        self.world_size
    }

    /// Get the primal mesh (quads with anchor edges as wires)
    pub fn primal(&self) -> WasmIndexedMesh {
        self.primal.clone()
    }

    /// Get the dual mesh (dual polygons)
    pub fn dual(&self) -> WasmIndexedMesh {
        self.dual.clone()
    }
}

/// Generate a hex quad mesh from a JSON config string.
#[wasm_bindgen]
pub fn generate_mesh(config_json: &str) -> Result<WasmPatchMesh, JsValue> {
    let config: MeshConfig = serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let world_size = config.world_size;

    // Step 1: Generate base mesh from selected mesher
    let (mut mesh, subdivision) = match config.mesher {
        MesherConfig::Patch { subdivision, orientation } => {
            let orient = match orientation.as_str() {
                "Even" => PatchOrientation::Even,
                "Odd" => PatchOrientation::Odd,
                _ => return Err(JsValue::from_str("orientation must be 'Even' or 'Odd'")),
            };
            let mut mesher = PatchMesher::new(subdivision, orient).with_size(world_size);
            (mesher.generate_uniform(), subdivision)
        } /*MesherConfig::Cdt { subdivision, interior_points } => {
              let rng = Xorshift32::new(config.seed).into_rc();
              let mut mesher = CdtMesher::new(subdivision, interior_points, rng).with_world_size(world_size);
              (mesher.generate(), subdivision)
          }
          MesherConfig::Lattice { subdivision } => {
              let rng = Xorshift32::new(config.seed).into_rc();
              let mut mesher = LatticeMesher::new(subdivision, rng).with_world_size(world_size);
              (mesher.generate(), subdivision)
          }*/
    };

    let validate = || {
        let anchor_sibdivision = (1 << subdivision) + 1;
        let validator = mesh.validator();
        validator.validate()?;
        validator.validate_regular_flat_top_hexagon(anchor_sibdivision, config.world_size * 0.001)?;
        Ok::<(), QuadError>(())
    };

    if let Err(err) = validate() {
        log::error!("Validation failed for seed {}: {:?}", config.seed, err);
        return Err(JsValue::from_str(&format!("Invalid mesh ({}): {:?}", config.seed, err)));
    }

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
    let primal = mesh.primal_extractor(Vec2::ZERO).build_internal_mesh_with_anchors();
    let dual = mesh.dual_extractor(Vec2::ZERO).build_internal_mesh();

    WasmPatchMesh {
        world_size,
        primal: primal.into(),
        dual: dual.into(),
    }
}
