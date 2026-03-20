use glam::Vec2;
use rand::RngExt;
use shine_game::math::{
    hex::{patch_mesh_to_svg, PatchMesher, PatchOrientation},
    rand::StableRng,
};
use std::fs;

struct SysRng(rand::rngs::ThreadRng);
impl SysRng {
    fn new() -> Self {
        Self(rand::rng())
    }
}
impl StableRng for SysRng {
    fn next_u32(&mut self) -> u32 {
        self.0.random()
    }
}

fn tmp_dir() -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("temp");
    fs::create_dir_all(&dir).unwrap();
    dir
}

const SUBDIVISION: u32 = 3;
const ORIENTATION: PatchOrientation = PatchOrientation::Even;

fn write_svg(vertices: &[Vec2], filename: &str) {
    let svg = patch_mesh_to_svg(vertices, SUBDIVISION, ORIENTATION);
    let path = tmp_dir().join(filename);
    fs::write(&path, svg).unwrap();
    println!("SVG written to {}", path.display());
}

#[test]
fn generate_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION, SysRng::new());
    let mut vertices = mesher.create_vertex_buffer();
    mesher.generate_uniform(&mut vertices);
    write_svg(&vertices, "hex_mesh_uniform.svg");
}

#[test]
fn generate_subdiv_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION, SysRng::new());
    let mut vertices = mesher.create_vertex_buffer();
    mesher.generate_subdivision(&mut vertices);
    write_svg(&vertices, "hex_mesh_subdiv_uniform.svg");
}

#[test]
fn generate_weighted_lloyd() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION, SysRng::new());
    let mut vertices = mesher.create_vertex_buffer();
    mesher.generate_uniform(&mut vertices);
    mesher.smooth_weighted_lloyd(20, 0.4, (2.5, 15.5), &mut vertices);
    write_svg(&vertices, "hex_mesh_lloyd.svg");
    mesher.fix_quads(0.15, 50, &mut vertices);
    write_svg(&vertices, "hex_mesh_lloyd_fixed.svg");
}

#[test]
fn generate_noise() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION, SysRng::new());
    let mut vertices = mesher.create_vertex_buffer();
    mesher.generate_uniform(&mut vertices);
    mesher.smooth_noise(0.5, 5.0, &mut vertices);
    write_svg(&vertices, "hex_mesh_noise.svg");
    mesher.fix_quads(0.15, 50, &mut vertices);
    write_svg(&vertices, "hex_mesh_noise_fixed.svg");
}

#[test]
fn generate_cotangent() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION, SysRng::new());
    let mut vertices = mesher.create_vertex_buffer();
    mesher.generate_uniform(&mut vertices);
    mesher.smooth_cotangent(10, 0.5, &mut vertices);
    write_svg(&vertices, "hex_mesh_cotangent.svg");
    mesher.fix_quads(0.15, 50, &mut vertices);
    write_svg(&vertices, "hex_mesh_cotangent_fixed.svg");
}

#[test]
fn generate_spring() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION, SysRng::new());
    let mut vertices = mesher.create_vertex_buffer();
    mesher.generate_uniform(&mut vertices);
    mesher.smooth_spring(50, 0.1, 0.3, 0.5, &mut vertices);
    write_svg(&vertices, "hex_mesh_spring.svg");
    mesher.fix_quads(0.15, 50, &mut vertices);
    write_svg(&vertices, "hex_mesh_spring_fixed.svg");
}

#[test]
fn generate_jitter() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION, SysRng::new());
    let mut vertices = mesher.create_vertex_buffer();
    mesher.generate_uniform(&mut vertices);
    mesher.smooth_jitter(2.0, &mut vertices);
    write_svg(&vertices, "hex_mesh_jitter.svg");
    mesher.fix_quads(0.15, 50, &mut vertices);
    write_svg(&vertices, "hex_mesh_jitter_fixed.svg");
}
