use glam::IVec2;
use shine_game::math::triangulation::Triangulation;

/// Helper to reduce a failing test case by trying to remove points and edges one by one while still keeping the test failing.
#[allow(dead_code)]
fn reduce_failing_case(points: Vec<IVec2>, edges: Vec<(usize, usize)>) -> (Vec<IVec2>, Vec<(usize, usize)>) {
    let is_failing_test = |pts: &[IVec2], eds: &[(usize, usize)]| -> bool {
        let pts = pts.to_vec();
        let eds = eds.to_vec();

        let mut tri = Triangulation::new_cdt();
        let mut builder = tri.builder();
        let mut vertices = Vec::new();

        for &pnt in &pts {
            let vi = builder.add_vertex(pnt, None);
            if builder.check().is_err() {
                return true;
            }
            vertices.push(vi);
        }

        for &(a, b) in &eds {
            if a >= vertices.len() || b >= vertices.len() {
                continue;
            }
            builder.add_constraint_edge(vertices[a], vertices[b], 1);
            if builder.check().is_err() {
                return true;
            }
        }
        return false;
    };

    if !is_failing_test(&points, &edges) {
        println!("Warning: reduce_failing_case called on a case that does not fail.");
        return (points, edges);
    }

    let mut current_points = points;
    let mut current_edges = edges;

    let mut removed = true;
    while removed {
        removed = false;

        // 1. Try to remove edges one by one
        let mut i = 0;
        while i < current_edges.len() {
            let mut next_edges = current_edges.clone();
            next_edges.remove(i);

            if is_failing_test(&current_points, &next_edges) {
                current_edges = next_edges;
                println!("Reduced to {} edges", current_edges.len());
                removed = true;
                i = 0;
            } else {
                i += 1;
            }
        }

        // 2. Try to remove points one by one
        let mut i = 0;
        while i < current_points.len() {
            if current_points.len() <= 3 {
                break;
            }

            let mut next_points = current_points.clone();
            next_points.remove(i);

            let mut next_edges = Vec::new();
            let mut possible = true;
            for &(a, b) in &current_edges {
                if a == i || b == i {
                    // If an edge relies on this point, we can only remove the point if we can remove the edge.
                    // Since we already minimized edges, we assume we can't remove this point if it's an endpoint.
                    possible = false;
                    break;
                }
                let new_a = if a > i { a - 1 } else { a };
                let new_b = if b > i { b - 1 } else { b };
                next_edges.push((new_a, new_b));
            }

            if possible && is_failing_test(&next_points, &next_edges) {
                current_points = next_points;
                current_edges = next_edges;
                println!("Reduced to {} points", current_points.len());
                removed = true;
                i = 0;
            } else {
                i += 1;
            }
        }
    }

    eprintln!(
        "let points: Vec<IVec2> = vec!{:?}.into_iter().map(|(x, y)| IVec2::new(x, y)).collect();",
        current_points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>()
    );
    eprintln!("let edges: Vec<(usize, usize)> = vec!{:?};", current_edges);

    (current_points, current_edges)
}

#[test]
fn fuzz_cdt_issue1() {
    let mut tri = Triangulation::new_cdt();
    let points: Vec<IVec2> = vec![IVec2::new(-3, 0), IVec2::new(3, 0), IVec2::new(0, 5), IVec2::new(0, -1)];
    let mut builder = tri.builder();
    let mut vertices = Vec::new();
    for &pnt in &points {
        vertices.push(builder.add_vertex(pnt, None));
    }
    builder.add_constraint_edge(vertices[0], vertices[1], 1);
    builder.add_constraint_edge(vertices[2], vertices[3], 1);
    assert_eq!(builder.check(), Ok(()));
}

#[test]
fn fuzz_cdt_issue2() {
    let mut tri = Triangulation::new_cdt();
    let points: Vec<IVec2> = vec![
        IVec2::new(0, 0),
        IVec2::new(10, 0),
        IVec2::new(10, 10),
        IVec2::new(0, 10),
        IVec2::new(5, 5),
    ];
    let mut builder = tri.builder();
    let mut vertices = Vec::new();
    for &pnt in &points {
        vertices.push(builder.add_vertex(pnt, None));
    }
    builder.add_constraint_edge(vertices[0], vertices[2], 1);
    builder.add_constraint_edge(vertices[1], vertices[3], 1);
    assert_eq!(builder.check(), Ok(()));
}

#[test]
fn fuzz_cdt_issue3() {
    let mut tri = Triangulation::new_cdt();
    let points: Vec<IVec2> = vec![
        IVec2::new(0, 0),
        IVec2::new(10, 0),
        IVec2::new(10, 10),
        IVec2::new(0, 10),
        IVec2::new(2, 2),
        IVec2::new(8, 2),
        IVec2::new(8, 8),
        IVec2::new(2, 8),
    ];
    let mut builder = tri.builder();
    let mut vertices = Vec::new();
    for &pnt in &points {
        vertices.push(builder.add_vertex(pnt, None));
    }
    builder.add_constraint_edge(vertices[4], vertices[5], 1);
    builder.add_constraint_edge(vertices[5], vertices[6], 1);
    builder.add_constraint_edge(vertices[6], vertices[7], 1);
    builder.add_constraint_edge(vertices[7], vertices[4], 1);
    assert_eq!(builder.check(), Ok(()));
}

#[test]
fn fuzz_cdt_issue4() {
    let points: Vec<IVec2> = vec![
        (87, 65),
        (-49, -1),
        (-44, 11),
        (-1, -1),
        (-70, -1),
        (-29, 15),
        (45, -1),
        (45, -71),
        (-69, -9),
        (-69, 87),
        (59, -1),
        (-20, -79),
        (-3, -1),
    ]
    .into_iter()
    .map(|(x, y)| IVec2::new(x, y))
    .collect();
    let edges: Vec<(usize, usize)> = vec![(0, 8), (4, 10), (10, 2)];

    let mut tri = Triangulation::new_cdt();
    let mut builder = tri.builder();
    let mut vertices = Vec::new();
    for &pnt in &points {
        vertices.push(builder.add_vertex(pnt, None));
        assert_eq!(builder.check(), Ok(()));
    }
    for &(a, b) in &edges {
        builder.add_constraint_edge(vertices[a], vertices[b], 1);
        assert_eq!(builder.check(), Ok(()));
    }
}

#[test]
fn fuzz_cdt_issue4_reduced() {
    let points: Vec<(i32, i32)> = vec![
        (87, 65),
        (-49, -1),
        (-1, -1),
        (-70, -1),
        (-29, 25),
        (-69, -9),
        (59, -1),
        (0, -79),
        (-13, -1),
    ];
    let edges: Vec<(usize, usize)> = vec![(0, 5), (3, 6)];

    let points: Vec<_> = points.into_iter().map(|(x, y)| IVec2::new(x, y)).collect();

    let mut tri = Triangulation::new_cdt();
    let mut builder = tri.builder().with_debug(usize::MAX, "../../temp/cdt/fuzz_cdt_issue5");
    let mut vertices = Vec::new();
    for &pnt in &points {
        vertices.push(builder.add_vertex(pnt, None));
        assert_eq!(builder.check(), Ok(()));
    }
    for &(a, b) in &edges {
        builder.add_constraint_edge(vertices[a], vertices[b], 1);
        assert_eq!(builder.check(), Ok(()));
    }
}

#[test]
fn fuzz_cdt_issue5() {
    let points: Vec<(i32, i32)> = vec![
        (-1, -1),
        (-1, -1),
        (87, 61),
        (87, -71),
        (-1, 75),
        (-81, 72),
        (-11, -1),
        (-45, -1),
        (41, -90),
        (66, 0),
        (-85, -49),
        (-1, 11),
    ];
    let edges: Vec<(usize, usize)> = vec![(2, 3), (5, 3), (10, 11)];

    let points: Vec<_> = points.into_iter().map(|(x, y)| IVec2::new(x, y)).collect();

    let mut tri = Triangulation::new_cdt();
    let mut builder = tri.builder();
    let mut vertices = Vec::new();
    for &pnt in &points {
        vertices.push(builder.add_vertex(pnt, None));
        assert_eq!(builder.check(), Ok(()));
    }
    for &(a, b) in &edges {
        builder.add_constraint_edge(vertices[a], vertices[b], 1);
        assert_eq!(builder.check(), Ok(()));
    }
}
