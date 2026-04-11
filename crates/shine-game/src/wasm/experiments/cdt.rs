use crate::{
    indexed::TypedIndex,
    math::{
        rand::{StableRng, Xorshift32},
        triangulation::{Rot3Idx, Triangulation, VertexIndex},
    },
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

    let mut tri = Triangulation::new_cdt();
    let mut vertex_indices: Vec<VertexIndex> = Vec::with_capacity(points.len());
    {
        let mut builder = tri.builder();
        // Add vertices
        for &p in &points {
            let vi = builder.add_vertex(p, None);
            vertex_indices.push(vi);
        }

        // Add boundary constraint edges
        for edge in &edges {
            let v0 = vertex_indices[edge.0];
            let v1 = vertex_indices[edge.1];
            builder.add_constraint_edge(v0, v1, 1);
        }
    }

    // Extract finite triangles
    let mut triangles: Vec<u32> = Vec::new();
    for f in tri.face_index_iter() {
        if tri.is_infinite_face(f) {
            continue;
        }

        let v0 = tri[f].vertices[Rot3Idx::new(0)];
        let v1 = tri[f].vertices[Rot3Idx::new(1)];
        let v2 = tri[f].vertices[Rot3Idx::new(2)];

        // Find the original indices
        let i0 = vertex_indices.iter().position(|&v| v == v0).unwrap() as u32;
        let i1 = vertex_indices.iter().position(|&v| v == v1).unwrap() as u32;
        let i2 = vertex_indices.iter().position(|&v| v == v2).unwrap() as u32;

        triangles.push(i0);
        triangles.push(i1);
        triangles.push(i2);
    }
    let fixed_edges: Vec<u32> = edges.iter().flat_map(|&(a, b)| [a as u32, b as u32]).collect();

    WasmCdt {
        vertices,
        triangles,
        fixed_edges,
        error: None,
    }
}
