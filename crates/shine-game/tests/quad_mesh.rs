use rand::rngs::StdRng;
use rand::SeedableRng;
use shine_core::utils::is_rotation;
use shine_game::{
    indexed::TypedIndex,
    math::quadrangulation::{
        AnchorIndex, EdgeCirculator, QuadEdge, QuadEdgeType, QuadIndex, Quadrangulation, RandomizationMap, Rot4Idx,
        VertexIndex,
    },
};
use shine_test::test;
use std::collections::HashSet;

fn randomized_grid_2x2() -> (Quadrangulation, RandomizationMap) {
    let mut rng = StdRng::seed_from_u64(42);
    let mut mesh = Quadrangulation::new_2x2_grid();
    let randomization_map = mesh.builder().randomize(&mut rng);
    (mesh, randomization_map)
}

#[test]
fn test_topology_counts() {
    let mesh = Quadrangulation::new_2x2_grid();

    assert_eq!(mesh.vertex_count(), 10, "10 total vertices (9 finite + 1 infinite)");
    assert_eq!(mesh.finite_vertex_count(), 9, "9 finite vertices");
    assert_eq!(mesh.quad_count(), 8, "8 total quads (4 finite + 4 infinite)");
    assert_eq!(mesh.finite_quad_count(), 4, "4 finite quads");
    assert_eq!(mesh.infinite_quad_count(), 4, "4 infinite quads (8 boundary edges / 2)");
    assert_eq!(mesh.boundary_vertex_count(), 8, "8 boundary vertices");

    let infinite_vertex = mesh.infinite_vertex();
    assert_eq!(infinite_vertex, VertexIndex::new(9), "infinite vertex at index 9");
}

#[test]
fn test_infinite_quad_structure() {
    let mesh = Quadrangulation::new_2x2_grid();

    let infinite_vertex = mesh.infinite_vertex();
    for qi in mesh.infinite_quad_index_iter() {
        let verts = mesh.quad_vertices(qi);

        // Infinite quad should have exactly one infinite vertex
        let infinite_count = verts.iter().filter(|&&v| v == infinite_vertex).count();
        assert_eq!(
            infinite_count, 1,
            "infinite quad {:?} should contain exactly 1 infinite vertex",
            qi
        );

        // Other 3 vertices should be finite boundary vertices
        let finite_verts: Vec<_> = verts.iter().filter(|&&v| v != infinite_vertex).copied().collect();
        assert_eq!(finite_verts.len(), 3, "infinite quad should have 3 finite vertices");

        for &v in &finite_verts {
            assert!(
                mesh.is_boundary_vertex(v),
                "finite vertex {:?} in infinite quad should be boundary",
                v
            );
        }
    }
}

#[test]
fn test_quad_neighbor_consistency() {
    let (mesh, _map) = randomized_grid_2x2();

    // Check ALL quads, including ghost quads
    for qi_idx in 0..mesh.quad_count() {
        let qi = QuadIndex::new(qi_idx);
        for edge in 0..4 {
            let qe = QuadEdge {
                quad: qi,
                edge: Rot4Idx::new(edge),
            };
            let twin = mesh.edge_twin(qe);
            let (v0, v1) = mesh.edge_vertices(qe);
            let (tv0, tv1) = mesh.edge_vertices(twin);

            // Twin should have reversed vertices
            assert_eq!(
                (v0, v1),
                (tv1, tv0),
                "quad {} edge {}: twin vertices should be reversed",
                qi_idx,
                edge
            );

            // Twin of twin should be the original
            let round_trip = mesh.edge_twin(twin);
            assert_eq!(
                round_trip.quad, qe.quad,
                "quad {} edge {}: twin involution (quad)",
                qi_idx, edge
            );
            assert_eq!(
                round_trip.edge, qe.edge,
                "quad {} edge {}: twin involution (edge)",
                qi_idx, edge
            );
        }
    }
}

#[test]
fn test_vertex_ring_interior() {
    let (mesh, map) = randomized_grid_2x2();

    // Interior vertex 4 ring should be [Q0, Q1, Q3, Q2] in some rotation
    let ring_ccw: Vec<_> = mesh.vertex_ring_ccw(map.vertex(4)).map(|qv| qv.quad).collect();
    let expected: Vec<_> = [0, 1, 3, 2].map(|q| map.quad(q)).to_vec();
    assert!(is_rotation(&expected, &ring_ccw));

    let ring_ccw: Vec<_> = mesh.vertex_ring_cw(map.vertex(4)).map(|qv| qv.quad).collect();
    let expected: Vec<_> = [2, 3, 1, 0].map(|q| map.quad(q)).to_vec();
    assert!(is_rotation(&expected, &ring_ccw));
}

#[test]
fn test_vertex_rings_consistency() {
    let (mesh, _) = randomized_grid_2x2();

    // All real vertices should have valid, connected rings referencing the correct vertex
    for vi in mesh.finite_vertex_index_iter() {
        let ring: Vec<_> = mesh.vertex_ring_ccw(vi).collect();
        assert!(!ring.is_empty(), "vertex {:?} should have non-empty ring", vi);

        for qv in &ring {
            assert_eq!(mesh.vi(*qv), vi, "ring entry should reference vertex {:?}", vi);
        }

        // Ring should be connected via incoming edge twins
        for i in 0..ring.len() {
            let current = ring[i];
            let next_in_ring = ring[(i + 1) % ring.len()];

            let incoming = current.incoming_edge();
            let neighbor = mesh.edge_twin(incoming);

            assert_eq!(
                neighbor.start(),
                next_in_ring,
                "neighbor.start() should match next in ring for vertex {:?}",
                vi
            );
        }
    }
}

#[test]
fn test_edge_circulator_ccw() {
    let (mesh, map) = randomized_grid_2x2();

    let mut circ = EdgeCirculator::new(&mesh, map.vertex(4));
    let start = circ.current();
    let mut visited = Vec::new();
    for _ in 0..4 {
        visited.push(circ.quad());
        circ.advance_ccw();
    }
    assert_eq!(circ.current(), start);

    let expected: Vec<_> = [0, 1, 3, 2].map(|q| map.quad(q)).to_vec();
    assert!(is_rotation(&expected, &visited));
}

#[test]
fn test_edge_circulator_cw() {
    let (mesh, map) = randomized_grid_2x2();

    let mut circ = EdgeCirculator::new(&mesh, map.vertex(4));
    let start = circ.current();
    let mut visited = Vec::new();
    for _ in 0..4 {
        visited.push(circ.current().quad);
        circ.advance_cw();
    }

    // Should return to start after 4 steps
    assert_eq!(
        circ.current(),
        start,
        "circulator should return to start after full CW loop"
    );
    assert_eq!(circ.current(), start);

    let expected: Vec<_> = [0, 2, 3, 1].map(|q| map.quad(q)).to_vec();
    assert!(is_rotation(&expected, &visited));
}

#[test]
fn test_edge_circulator_mixed_direction() {
    let (mesh, map) = randomized_grid_2x2();

    let mut circ = EdgeCirculator::new(&mesh, map.vertex(4));
    let start = circ.current();

    // Go 2 step ccw, cw should return to start
    circ.advance_ccw();
    circ.advance_ccw();
    let pos_after_2ccw = circ.current();
    circ.advance_cw();
    circ.advance_cw();
    assert_eq!(circ.current(), start);

    // Go 1 step ccw, cw should return to start
    circ.advance_cw();
    let pos_after_1cw = circ.current();
    circ.advance_ccw();
    assert_eq!(circ.current(), start);

    // Positions after 2ccw and 1cw should be different from start
    assert_ne!(pos_after_2ccw, start);
    assert_ne!(pos_after_1cw, start);
}

#[test]
fn test_boundary_detection() {
    let mesh = Quadrangulation::new_2x2_grid();

    let expected_boundary: HashSet<_> = [0, 1, 2, 3, 5, 6, 7, 8].into_iter().collect();
    let expected_interior: HashSet<_> = [4].into_iter().collect();

    for vi in mesh.finite_vertex_index_iter() {
        let idx = vi.into_index();
        let is_boundary = mesh.is_boundary_vertex(vi);

        if expected_boundary.contains(&idx) {
            assert!(is_boundary, "vertex {} should be boundary", idx);
        } else {
            assert!(!is_boundary, "vertex {} should be interior", idx);
        }

        // Cross-check: is_boundary_vertex matches ring-has-ghost
        let ring_has_ghost = mesh.vertex_ring_ccw(vi).any(|qv| mesh.is_infinite_quad(qv.quad));
        assert_eq!(
            is_boundary, ring_has_ghost,
            "vertex {}: is_boundary_vertex and ring_has_ghost should match",
            idx
        );
    }

    assert!(expected_interior.contains(&4));
}

#[test]
fn test_edge_classification_interior() {
    let mesh = Quadrangulation::new_2x2_grid();

    // Interior vertex 4 should have all interior edges to its neighbors
    let infinite_vertex = mesh.infinite_vertex();
    for qv in mesh.vertex_ring_ccw(VertexIndex::new(4)) {
        let next_v = mesh.vi(qv.next());
        if next_v != infinite_vertex {
            assert_eq!(
                mesh.edge_type(VertexIndex::new(4), next_v),
                QuadEdgeType::Interior,
                "edge from interior vertex 4 to {:?} should be Interior",
                next_v
            );
        }
    }
}

#[test]
fn test_edge_classification_boundary() {
    let mesh = Quadrangulation::new_2x2_grid();

    // Boundary edges should be Boundary in both directions
    let boundary_pairs = [(0, 1), (1, 2), (2, 5), (5, 8), (8, 7), (7, 6), (6, 3), (3, 0)];
    for (a, b) in boundary_pairs {
        assert_eq!(
            mesh.edge_type(VertexIndex::new(a), VertexIndex::new(b)),
            QuadEdgeType::Boundary,
            "edge {}→{} should be Boundary",
            a,
            b
        );
        assert_eq!(
            mesh.edge_type(VertexIndex::new(b), VertexIndex::new(a)),
            QuadEdgeType::Boundary,
            "edge {}→{} (reverse) should be Boundary",
            b,
            a
        );
    }
}

#[test]
fn test_edge_classification_not_an_edge() {
    let mesh = Quadrangulation::new_2x2_grid();

    // Diagonal vertices that share no edge
    assert_eq!(
        mesh.edge_type(VertexIndex::new(0), VertexIndex::new(4)),
        QuadEdgeType::NotAnEdge,
        "diagonal 0→4 should be NotAnEdge"
    );
    assert_eq!(
        mesh.edge_type(VertexIndex::new(0), VertexIndex::new(8)),
        QuadEdgeType::NotAnEdge,
        "non-adjacent 0→8 should be NotAnEdge"
    );
}

#[test]
fn test_anchor_edges_ccw_order() {
    let mesh = Quadrangulation::new_2x2_grid();

    // Edge 0: anchor 0 -> 2, should be [0, 1, 2]
    let edge0: Vec<_> = mesh.anchor_edge(AnchorIndex::new(0)).collect();
    let expected0: Vec<_> = [0, 1, 2].into_iter().map(VertexIndex::new).collect();
    assert_eq!(edge0, expected0, "anchor edge 0 should be [0, 1, 2]");

    // Edge 1: anchor 2 -> 8, should be [2, 5, 8]
    let edge1: Vec<_> = mesh.anchor_edge(AnchorIndex::new(1)).collect();
    let expected1: Vec<_> = [2, 5, 8].into_iter().map(VertexIndex::new).collect();
    assert_eq!(edge1, expected1, "anchor edge 1 should be [2, 5, 8]");

    // Edge 2: anchor 8 -> 6, should be [8, 7, 6]
    let edge2: Vec<_> = mesh.anchor_edge(AnchorIndex::new(2)).collect();
    let expected2: Vec<_> = [8, 7, 6].into_iter().map(VertexIndex::new).collect();
    assert_eq!(edge2, expected2, "anchor edge 2 should be [8, 7, 6]");

    // Edge 3: anchor 6 -> 0 (wrapping), should be [6, 3, 0]
    let edge3: Vec<_> = mesh.anchor_edge(AnchorIndex::new(3)).collect();
    let expected3: Vec<_> = [6, 3, 0].into_iter().map(VertexIndex::new).collect();
    assert_eq!(edge3, expected3, "anchor edge 3 should be [6, 3, 0]");
}

#[test]
fn test_average_adjacent_positions_interior() {
    let mesh = Quadrangulation::new_2x2_grid();

    // Interior vertex 4 at (1,1) has neighbors 1,3,5,7
    // Avg = ((1,0) + (0,1) + (2,1) + (1,2)) / 4 = (4,4)/4 = (1,1)
    let avg = mesh.average_adjacent_positions(VertexIndex::new(4));
    assert!((avg.x - 1.0).abs() < 0.001, "avg.x should be 1.0, got {}", avg.x);
    assert!((avg.y - 1.0).abs() < 0.001, "avg.y should be 1.0, got {}", avg.y);
}

#[test]
fn test_average_adjacent_positions_boundary() {
    let mesh = Quadrangulation::new_2x2_grid();

    // Boundary vertex 1 at (1,0) has real neighbors: 0, 2, 4 (ghost vertex is skipped)
    // Avg = ((0,0) + (2,0) + (1,1)) / 3 = (3,1)/3 = (1, 0.333...)
    let avg = mesh.average_adjacent_positions(VertexIndex::new(1));
    assert!((avg.x - 1.0).abs() < 0.001, "avg.x should be 1.0, got {}", avg.x);
    assert!(
        (avg.y - 1.0 / 3.0).abs() < 0.001,
        "avg.y should be ~0.333, got {}",
        avg.y
    );
}

#[test]
fn test_quad_centers_computed() {
    let mesh = Quadrangulation::new_2x2_grid();

    assert_eq!(mesh.finite_quad_count(), 4);

    // First quad center: average of (0,0), (1,0), (1,1), (0,1) = (0.5, 0.5)
    let center0 = mesh.dual_p(QuadIndex::new(0)).unwrap();
    assert!((center0.x - 0.5).abs() < 0.001);
    assert!((center0.y - 0.5).abs() < 0.001);

    // Second quad center: average of (1,0), (2,0), (2,1), (1,1) = (1.5, 0.5)
    let center1 = mesh.dual_p(QuadIndex::new(1)).unwrap();
    assert!((center1.x - 1.5).abs() < 0.001);
    assert!((center1.y - 0.5).abs() < 0.001);
}

#[test]
fn test_quad_centers_count_matches_real_quads() {
    let mesh = Quadrangulation::new_2x2_grid();

    assert_eq!(mesh.finite_quad_count(), 4);
}

#[test]
fn test_boundary_dual_vertices() {
    let (mesh, map) = randomized_grid_2x2();

    let v0_quads: Vec<_> = mesh.boundary_dual_vertices(map.vertex(0)).collect();
    assert_eq!(v0_quads, [0].map(|q| map.quad(q)));

    let v1_quads: Vec<_> = mesh.boundary_dual_vertices(map.vertex(1)).collect();
    assert_eq!(v1_quads, [1, 0].map(|q| map.quad(q)));

    let v2_quads: Vec<_> = mesh.boundary_dual_vertices(map.vertex(2)).collect();
    assert_eq!(v2_quads, [1].map(|q| map.quad(q)));

    let v3_quads: Vec<_> = mesh.boundary_dual_vertices(map.vertex(3)).collect();
    assert_eq!(v3_quads, [0, 2].map(|q| map.quad(q)));

    let v5_quads: Vec<_> = mesh.boundary_dual_vertices(map.vertex(5)).collect();
    assert_eq!(v5_quads, [3, 1].map(|q| map.quad(q)));

    let v6_quads: Vec<_> = mesh.boundary_dual_vertices(map.vertex(6)).collect();
    assert_eq!(v6_quads, [2].map(|q| map.quad(q)));

    let v7_quads: Vec<_> = mesh.boundary_dual_vertices(map.vertex(7)).collect();
    assert_eq!(v7_quads, [2, 3].map(|q| map.quad(q)));

    let v8_quads: Vec<_> = mesh.boundary_dual_vertices(map.vertex(8)).collect();
    assert_eq!(v8_quads, [3].map(|q| map.quad(q)));

    // Test interior vertex 4 (center)
    let v4_quads: Vec<_> = mesh.boundary_dual_vertices(map.vertex(4)).collect();
    let expected = [0, 1, 3, 2].map(|q| map.quad(q));
    assert!(is_rotation(&expected, &v4_quads));
}
