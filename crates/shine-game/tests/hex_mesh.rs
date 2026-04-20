use shine_game::math::{
    hex::{CdtMesher, LatticeMesher, PatchMesher, PatchOrientation},
    prng::{StableRng, SysRng},
};
use shine_test::test;

const SUBDIVISION: u32 = 3;
const ORIENTATION: PatchOrientation = PatchOrientation::Even;
/// Expected vertices per anchor edge: 2^SUBDIVISION + 1
const EXPECTED_ANCHOR_EDGE_LEN: usize = (1 << SUBDIVISION) + 1;

fn check_anchor_edges(mesh: &shine_game::math::quadrangulation::QuadMesh) {
    assert_eq!(
        mesh.topology.anchor_count(),
        6,
        "hex mesh should have 6 anchor vertices"
    );
    for i in 0..6 {
        let edge_len = mesh.topology.anchor_edge(i).count();
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
    mesh.topology.validate().expect("uniform mesh topology should be valid");
    assert!(mesh.topology.quad_count() > 0, "uniform mesh should have quads");
    check_anchor_edges(&mesh);
}

#[test]
fn generate_subdiv_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mesh = mesher.generate_subdivision();
    mesh.topology
        .validate()
        .expect("subdivision mesh topology should be valid");
    assert!(mesh.topology.quad_count() > 0, "subdivision mesh should have quads");
    check_anchor_edges(&mesh);
}

#[test]
fn generate_cdt_mesh() {
    let mut mesher = CdtMesher::new(SUBDIVISION, 20, SysRng::new().into_rc());
    let mesh = mesher.generate();
    mesh.topology.validate().expect("CDT mesh topology should be valid");
    assert!(mesh.topology.quad_count() > 0, "CDT mesh should have quads");
    check_anchor_edges(&mesh);
}

#[test]
fn generate_lattice_mesh() {
    let mut mesher = LatticeMesher::new(SUBDIVISION, SysRng::new().into_rc());
    let mesh = mesher.generate();
    mesh.topology.validate().expect("lattice mesh topology should be valid");
    assert!(mesh.topology.quad_count() > 0, "lattice mesh should have quads");
    check_anchor_edges(&mesh);
}
