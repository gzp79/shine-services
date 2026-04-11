use glam::IVec2;
use shine_game::math::triangulation::{GeometryChecker, TopologyChecker, Triangulation};
use shine_test::test;

#[test]
fn delaunay_checker_passes() {
    let mut tri = Triangulation::new_cdt();

    // Create a Delaunay triangulation
    let mut builder = tri.builder();
    builder.add_vertex(IVec2::new(0, 0), None);
    builder.add_vertex(IVec2::new(100, 0), None);
    builder.add_vertex(IVec2::new(50, 50), None);
    builder.add_vertex(IVec2::new(50, 10), None);
    drop(builder);

    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
}

#[test]
fn delaunay_checker_with_constraints() {
    let mut tri = Triangulation::new_cdt();

    // Create vertices

    let mut builder = tri.builder();
    let v0 = builder.add_vertex(IVec2::new(0, 0), None);
    builder.add_vertex(IVec2::new(100, 0), None);
    let v2 = builder.add_vertex(IVec2::new(100, 100), None);
    builder.add_vertex(IVec2::new(0, 100), None);
    builder.add_vertex(IVec2::new(50, 50), None);

    // Add a constraint diagonal - this may violate Delaunay but it's allowed
    builder.add_constraint_edge(v0, v2, 1);
    drop(builder);

    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
}

#[test]
fn constrained_delaunay_restoration() {
    let mut tri = Triangulation::new_cdt();

    // Create a square with a center point
    let mut builder = tri.builder();

    let v0 = builder.add_vertex(IVec2::new(0, 0), None);
    builder.add_vertex(IVec2::new(100, 0), None);
    let v2 = builder.add_vertex(IVec2::new(100, 100), None);
    builder.add_vertex(IVec2::new(0, 100), None);
    builder.add_vertex(IVec2::new(50, 50), None);

    // Add a constraint that will force re-triangulation
    // The Delaunay restoration should kick in and optimize the non-constrained edges
    builder.add_constraint_edge(v0, v2, 1);
    drop(builder);

    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
}

#[test]
fn multiple_constraints_with_delaunay() {
    let mut tri = Triangulation::new_cdt();

    // Create a grid of points
    let mut builder = tri.builder();

    let v00 = builder.add_vertex(IVec2::new(0, 0), None);
    let v10 = builder.add_vertex(IVec2::new(100, 0), None);
    let v20 = builder.add_vertex(IVec2::new(200, 0), None);
    let v01 = builder.add_vertex(IVec2::new(0, 100), None);
    builder.add_vertex(IVec2::new(100, 100), None);
    let v21 = builder.add_vertex(IVec2::new(200, 100), None);
    let v02 = builder.add_vertex(IVec2::new(0, 200), None);
    let v12 = builder.add_vertex(IVec2::new(100, 200), None);
    let v22 = builder.add_vertex(IVec2::new(200, 200), None);

    // Add multiple constraint edges
    builder.add_constraint_edge(v00, v22, 1);
    builder.add_constraint_edge(v20, v02, 2);
    builder.add_constraint_edge(v10, v12, 4);
    builder.add_constraint_edge(v01, v21, 8);
    drop(builder);

    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
}
