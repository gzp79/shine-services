use rand::RngExt;
use shine_game::math::{
    hex::{CdtMesher, LatticeMesher, PatchMesher, PatchOrientation},
    mesh::{Jitter, LaplacianSmoother, QuadFilter, QuadRelax, VertexRepulsion},
    rand::StableRng,
};

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

const SUBDIVISION: u32 = 3;
const ORIENTATION: PatchOrientation = PatchOrientation::Even;

#[test]
fn generate_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mesh = mesher.generate_uniform();
    println!(
        "Uniform: {} vertices, {} quads",
        mesh.topology.vertex_count(),
        mesh.topology.quad_count()
    );
}

#[test]
fn generate_subdiv_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mesh = mesher.generate_subdivision();
    println!(
        "Subdiv: {} vertices, {} quads",
        mesh.topology.vertex_count(),
        mesh.topology.quad_count()
    );
}

#[test]
fn generate_with_laplacian() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mut mesh = mesher.generate_uniform();
    let mut smoother = LaplacianSmoother::new(0.5, 20);
    smoother.apply(&mut mesh);
    println!(
        "Laplacian: {} vertices, {} quads",
        mesh.topology.vertex_count(),
        mesh.topology.quad_count()
    );
}

#[test]
fn generate_with_jitter() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mut mesh = mesher.generate_uniform();
    let mut jitter = Jitter::new(0.3, SysRng::new());
    jitter.apply(&mut mesh);
    println!(
        "Jitter: {} vertices, {} quads",
        mesh.topology.vertex_count(),
        mesh.topology.quad_count()
    );
}

/// Composable pipeline: jitter, then laplacian, then quad relax.
#[test]
fn generate_with_filter_pipeline() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mut mesh = mesher.generate_uniform();

    let mut filters: Vec<Box<dyn QuadFilter>> = vec![
        Box::new(Jitter::new(0.3, SysRng::new())),
        Box::new(LaplacianSmoother::new(0.5, 10)),
        Box::new(QuadRelax::new(0.15, 0.5, 50)),
    ];

    for f in &mut filters {
        f.apply(&mut mesh);
    }

    println!(
        "Pipeline: {} vertices, {} quads",
        mesh.topology.vertex_count(),
        mesh.topology.quad_count()
    );
}

#[test]
fn generate_cdt_mesh() {
    let mut mesher = CdtMesher::new(4, 20, SysRng::new());
    let mesh = mesher.generate();
    println!(
        "CDT mesh: {} vertices, {} quads",
        mesh.topology.vertex_count(),
        mesh.topology.quad_count()
    );
}

#[test]
fn generate_lattice_mesh() {
    let mut mesher = LatticeMesher::new(2, SysRng::new());
    let mesh = mesher.generate();
    println!(
        "Lattice mesh: {} vertices, {} quads",
        mesh.topology.vertex_count(),
        mesh.topology.quad_count()
    );
    assert!(mesh.topology.quad_count() > 0);
}

#[test]
fn generate_with_vertex_repulsion() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mut mesh = mesher.generate_uniform();
    let mut repulsion = VertexRepulsion::new(0.2, 10);
    repulsion.apply(&mut mesh);
    println!(
        "VertexRepulsion: {} vertices, {} quads",
        mesh.topology.vertex_count(),
        mesh.topology.quad_count()
    );
}

#[test]
fn generate_cdt_with_laplacian() {
    let mut mesher = CdtMesher::new(4, 20, SysRng::new());
    let mut mesh = mesher.generate();
    let mut smoother = LaplacianSmoother::new(0.5, 20);
    smoother.apply(&mut mesh);
    println!(
        "CDT + Laplacian: {} vertices, {} quads",
        mesh.topology.vertex_count(),
        mesh.topology.quad_count()
    );
}
