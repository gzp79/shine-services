use shine_game::{
    indexed::TypedIndex,
    math::{
        hex::{CdtMesher, LatticeMesher, PatchMesher, PatchOrientation},
        prng::{StableRng, SysRng, Xorshift32},
        quadrangulation::Quadrangulation,
    },
};
use shine_test::test;

const SUBDIVISION: u32 = 2; // Minimum subdivision for even edge count between anchors
const INTERNAL_POINTS: u32 = 100; // Minimum subdivision for even edge count between anchors
const ORIENTATION: PatchOrientation = PatchOrientation::Even;
/// Expected vertices per anchor edge: 2^SUBDIVISION + 1
const ANCHOR_SUBDIVISION: usize = (1 << SUBDIVISION) + 1;
const WORLD_SIZE: f32 = 42.0;

fn assert_valid_hexagon_mesh(mesh: &Quadrangulation) {
    let validator = mesh.validator();
    assert_eq!(validator.validate(), Ok(()));
    assert_eq!(
        validator.validate_regular_flat_top_hexagon(ANCHOR_SUBDIVISION, WORLD_SIZE, 1e-6),
        Ok(())
    );
}

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
fn generate_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION).with_size(WORLD_SIZE);
    let mesh = mesher.generate_uniform();
    assert_valid_hexagon_mesh(&mesh);
}

#[test]
fn test_uniform_determinism() {
    assert_mesher_deterministic("Patch", |_seed| {
        let mut mesher = PatchMesher::new(SUBDIVISION, PatchOrientation::Odd).with_size(WORLD_SIZE);
        mesher.generate_uniform()
    });
}

#[test]
fn generate_subdiv_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION).with_size(WORLD_SIZE);
    let mesh = mesher.generate_subdivision();
    assert_valid_hexagon_mesh(&mesh);
}

#[test]
fn generate_cdt_mesh() {
    let mut mesher = CdtMesher::new(SUBDIVISION, INTERNAL_POINTS, SysRng::new().into_rc()).with_size(WORLD_SIZE);
    let mesh = mesher.generate();
    assert_valid_hexagon_mesh(&mesh);
}

#[test]
fn test_cdt_determinism() {
    assert_mesher_deterministic("CDT", |seed| {
        let rng = Xorshift32::new(seed).into_rc();
        let mut mesher = CdtMesher::new(SUBDIVISION, INTERNAL_POINTS, rng).with_size(WORLD_SIZE);
        mesher.generate()
    });
}

#[test]
fn generate_lattice() {
    let mut mesher = LatticeMesher::new(SUBDIVISION, SysRng::new().into_rc()).with_size(WORLD_SIZE);
    let mesh = mesher.generate();
    assert_valid_hexagon_mesh(&mesh);
}

#[test]
fn test_lattice_determinism() {
    assert_mesher_deterministic("Lattice", |seed| {
        let rng = Xorshift32::new(seed).into_rc();
        let mut mesher = LatticeMesher::new(2, rng).with_size(WORLD_SIZE);
        mesher.generate()
    });
}
