use glam::IVec2;
use shine_game::math::triangulation::{GeometryChecker, TopologyChecker, Triangulation};
use shine_test::test;

#[test]
fn issue39_1() {
    let mut tri = Triangulation::new_ct();

    let pnts = vec![(0, 0), (2, 0), (1, 2), (-3, -3)];
    for &(x, y) in pnts.iter() {
        tri.builder().add_vertex(IVec2::new(x, y), None);
    }

    assert_eq!(tri.dimension(), 2);
    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
}

#[test]
fn issue39_2() {
    let mut tri = Triangulation::new_ct();

    let pnts = vec![(0, 0), (0, 1), (-1, 0), (1, 3)];
    for &(x, y) in pnts.iter() {
        tri.builder().add_vertex(IVec2::new(x, y), None);
    }
    assert_eq!(tri.dimension(), 2);
    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
}

#[test]
fn fuzz_constrained_delaunay_issue1() {
    let mut tri = Triangulation::new_cdt();

    let points = vec![(-20481, -20561), (-245, -3974), (5631, 5397), (5397, 5397), (-1, -1)];

    let mut builder = tri.builder();

    let mut vertices = Vec::new();
    for &(x, y) in &points {
        let vi = builder.add_vertex(IVec2::new(x, y), None);
        vertices.push(vi);
    }

    // Add constraint edge between vertices 0 and 3
    builder.add_constraint_edge(vertices[0], vertices[3], 1);
    drop(builder);

    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
}

#[test]
fn fuzz_constrained_issue2() {
    // Fuzz case: ConstrainedInput with 6 points and 2 edges
    // Points: [(-110, -1), (31487, -16), (2837, -4075), (5631, -3974), (-4075, -5), (-3974, -1)]
    // Edges: (133, 34) -> (1, 4), (15, 0) -> (3, 0)
    let mut tri = Triangulation::new_cdt();

    let points = vec![
        (-110, -1),
        (31487, -16),
        (2837, -4075),
        (5631, -3974),
        (-4075, -5),
        (-3974, -1),
    ];

    // Add vertices with Delaunay enabled (matching fuzzer behavior)
    let mut builder = tri.builder();
    let mut vertices = Vec::new();
    for &(x, y) in &points {
        let vi = builder.add_vertex(IVec2::new(x, y), None);
        vertices.push(vi);
    }
    drop(builder);

    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

    let n = vertices.len();
    let edges = [(133u8, 34u8), (15u8, 0u8)];

    for &(a, b) in &edges {
        let a = a as usize % n;
        let b = b as usize % n;
        if a == b {
            continue;
        }

        let mut builder = tri.builder();
        builder.add_constraint_edge(vertices[a], vertices[b], 1);
        drop(builder);

        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
    }
}

#[test]
fn fuzz_constrained_issue3() {
    // Fuzz case: ConstrainedInput with 17 points and 11 edges
    let mut tri = Triangulation::new_cdt();

    let points = vec![
        (-134, -3841),
        (-165, -1),
        (-1, -4),
        (-32473, 9185),
        (-725, -358),
        (-1, 9255),
        (-27757, 10131),
        (-1, -1),
        (-1, -9253),
        (-18213, -9423),
        (-9253, -37),
        (-1, -1),
        (-9217, -1),
        (-1, -1),
        (-9253, 9179),
        (7460, 8995),
        (-8, -1),
    ];

    // Add vertices with Delaunay enabled (matching fuzzer behavior)
    let mut builder = tri.builder();
    let mut vertices = Vec::new();
    for &(x, y) in &points {
        let vi = builder.add_vertex(IVec2::new(x, y), None);
        vertices.push(vi);
    }
    drop(builder);

    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

    let n = vertices.len();
    let edges = [
        (91u8, 255u8),
        (255u8, 50u8),
        (255u8, 252u8),
        (39u8, 81u8),
        (255u8, 0u8),
        (122u8, 240u8),
        (21u8, 11u8),
        (240u8, 39u8),
        (225u8, 255u8),
        (39u8, 129u8),
        (0u8, 0u8),
    ];

    for &(a, b) in &edges {
        let a = a as usize % n;
        let b = b as usize % n;
        if a == b {
            continue;
        }

        let mut builder = tri.builder();
        builder.add_constraint_edge(vertices[a], vertices[b], 1);
        drop(builder);

        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
    }
}
