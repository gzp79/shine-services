use crate::math::triangulation::Triangulation;
use glam::IVec2;

/// Helper to reduce a failing test case by trying to remove points and edges one by one.
pub fn reduce_test_case<F>(
    points: Vec<IVec2>,
    edges: Vec<(usize, usize)>,
    is_failing_test: F,
) -> (Vec<IVec2>, Vec<(usize, usize)>)
where
    F: Fn(&[IVec2], &[(usize, usize)]) -> bool,
{
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

pub fn reduce_test_case_incremental<const DELAUNAY: bool>(
    points: Vec<IVec2>,
    edges: Vec<(usize, usize)>,
) -> (Vec<IVec2>, Vec<(usize, usize)>) {
    reduce_test_case(points, edges, |pts, eds| {
        let pts = pts.to_vec();
        let eds = eds.to_vec();

        let mut tri = Triangulation::<DELAUNAY>::new();
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
    })
}

pub fn reduce_test_case_incremental_ct(
    points: Vec<IVec2>,
    edges: Vec<(usize, usize)>,
) -> (Vec<IVec2>, Vec<(usize, usize)>) {
    reduce_test_case_incremental::<false>(points, edges)
}

pub fn reduce_test_case_incremental_cdt(
    points: Vec<IVec2>,
    edges: Vec<(usize, usize)>,
) -> (Vec<IVec2>, Vec<(usize, usize)>) {
    reduce_test_case_incremental::<false>(points, edges)
}
