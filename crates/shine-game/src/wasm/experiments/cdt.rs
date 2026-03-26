use crate::math::{
    cdt::Triangulation,
    rand::{StableRng, Xorshift32},
};
use glam::IVec2;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

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
