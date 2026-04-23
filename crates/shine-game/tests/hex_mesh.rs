use shine_game::{
    indexed::TypedIndex,
    math::{
        hex::{CdtMesher, LatticeMesher, PatchMesher, PatchOrientation},
        prng::{StableRng, SysRng, Xorshift32},
        quadrangulation::{AnchorIndex, Quadrangulation},
    },
};
use shine_test::test;

const SUBDIVISION: u32 = 3;
const ORIENTATION: PatchOrientation = PatchOrientation::Even;
/// Expected vertices per anchor edge: 2^SUBDIVISION + 1
const EXPECTED_ANCHOR_EDGE_LEN: usize = (1 << SUBDIVISION) + 1;

fn check_anchor_edges(mesh: &Quadrangulation) {
    assert_eq!(mesh.anchor_count(), 6, "hex mesh should have 6 anchor vertices");
    for i in 0..6 {
        let edge_len = mesh.anchor_edge(AnchorIndex::new(i)).count();
        assert_eq!(
            edge_len, EXPECTED_ANCHOR_EDGE_LEN,
            "anchor edge {i} should have {EXPECTED_ANCHOR_EDGE_LEN} vertices, got {edge_len}"
        );
    }
}

#[test]
fn generate_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mesh = mesher.generate_uniform();
    mesh.validate().expect("uniform mesh topology should be valid");
    assert!(mesh.quad_count() > 0, "uniform mesh should have quads");
    check_anchor_edges(&mesh);
}

#[test]
fn generate_subdiv_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mesh = mesher.generate_subdivision();
    mesh.validate().expect("subdivision mesh topology should be valid");
    assert!(mesh.quad_count() > 0, "subdivision mesh should have quads");
    check_anchor_edges(&mesh);
}

#[test]
fn generate_cdt_mesh() {
    let mut mesher = CdtMesher::new(SUBDIVISION, 20, SysRng::new().into_rc());
    let mesh = mesher.generate();
    mesh.validate().expect("CDT mesh topology should be valid");
    assert!(mesh.quad_count() > 0, "CDT mesh should have quads");
    check_anchor_edges(&mesh);
}

#[test]
fn generate_lattice_mesh() {
    let mut mesher = LatticeMesher::new(SUBDIVISION, SysRng::new().into_rc());
    let mesh = mesher.generate();
    mesh.validate().expect("lattice mesh topology should be valid");
    assert!(mesh.quad_count() > 0, "lattice mesh should have quads");
    check_anchor_edges(&mesh);
}

/// Generic helper to verify mesher determinism by generating the same mesh twice
fn assert_mesher_deterministic<F>(name: &str, mut generate_mesh: F)
where
    F: FnMut(u32) -> Quadrangulation,
{
    let seed = SysRng::new().next_u32();

    let mesh1 = generate_mesh(seed);
    let mesh2 = generate_mesh(seed);

    assert_eq!(
        mesh1.vertex_count(),
        mesh2.vertex_count(),
        "{} mesher: vertex count differs",
        name
    );
    assert_eq!(
        mesh1.quad_count(),
        mesh2.quad_count(),
        "{} mesher: quad count differs",
        name
    );

    // Compare first 10 finite vertices by positions
    for vi in mesh1.finite_vertex_index_iter().take(10) {
        assert!(
            mesh2.is_finite_vertex(vi),
            "{} mesher: vertex {} is finite in mesh1 but not in mesh2",
            name,
            vi.into_index()
        );
        let p1 = mesh1.p(vi);
        let p2 = mesh2.p(vi);
        assert!(
            (p1 - p2).length() < 0.0001,
            "{} mesher: vertex {} position differs: {:?} vs {:?}",
            name,
            vi.into_index(),
            p1,
            p2
        );
    }
}

#[test]
fn test_patch_mesher_determinism() {
    assert_mesher_deterministic("Patch", |_seed| {
        let mut mesher = PatchMesher::new(2, PatchOrientation::Odd);
        mesher.generate_uniform()
    });
}

#[test]
fn test_lattice_mesher_determinism() {
    assert_mesher_deterministic("Lattice", |seed| {
        let rng = Xorshift32::new(seed).into_rc();
        let mut mesher = LatticeMesher::new(2, rng);
        mesher.generate()
    });
}

#[test]
fn test_cdt_mesher_determinism() {
    assert_mesher_deterministic("CDT", |seed| {
        let rng = Xorshift32::new(seed).into_rc();
        let mut mesher = CdtMesher::new(2, 10, rng);
        mesher.generate()
    });
}
