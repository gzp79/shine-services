use shine_game::{
    indexed::TypedIndex,
    math::{
        hex::{CdtMesher, LatticeMesher, PatchMesher, PatchOrientation},
        prng::{StableRng, SysRng, Xorshift32},
        quadrangulation::Quadrangulation,
    },
};
use shine_test::test;

const SUBDIVISION: u32 = 3;
const ORIENTATION: PatchOrientation = PatchOrientation::Even;
/// Expected vertices per anchor edge: 2^SUBDIVISION + 1
const ANCHOR_SUBDIVISION: usize = (1 << SUBDIVISION) + 1;

#[test]
fn generate_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mesh = mesher.generate_uniform();
    assert!(mesh.quad_count() > 0, "uniform mesh should have quads");
    let validator = mesh.validator();
    assert_eq!(validator.validate(), Ok(()));
    assert_eq!(
        validator.validate_regular_flat_top_hexagon(ANCHOR_SUBDIVISION, 1e-6),
        Ok(())
    );
}

#[test]
fn generate_subdiv_uniform() {
    let mut mesher = PatchMesher::new(SUBDIVISION, ORIENTATION);
    let mesh = mesher.generate_subdivision();
    assert!(mesh.quad_count() > 0, "subdivision mesh should have quads");
    let validator = mesh.validator();
    assert_eq!(validator.validate(), Ok(()));
    assert_eq!(
        validator.validate_regular_flat_top_hexagon(ANCHOR_SUBDIVISION, 1e-6),
        Ok(())
    );
}

#[test]
fn generate_cdt_mesh() {
    let mut mesher = CdtMesher::new(SUBDIVISION, 20, SysRng::new().into_rc());
    let mesh = mesher.generate();
    assert!(mesh.quad_count() > 0, "CDT mesh should have quads");
    let validator = mesh.validator();
    assert_eq!(validator.validate(), Ok(()));
    assert_eq!(
        validator.validate_regular_flat_top_hexagon(ANCHOR_SUBDIVISION, 1e-6),
        Ok(())
    );
}

#[test]
fn generate_lattice_mesh() {
    let mut mesher = LatticeMesher::new(SUBDIVISION, SysRng::new().into_rc());
    let mesh = mesher.generate();
    assert!(mesh.quad_count() > 0, "lattice mesh should have quads");
    let validator = mesh.validator();
    assert_eq!(validator.validate(), Ok(()));
    assert_eq!(
        validator.validate_regular_flat_top_hexagon(ANCHOR_SUBDIVISION, 1e-6),
        Ok(())
    );
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
