use glam::Vec2;
use shine_core::utils::is_rotation;
use shine_game::{
    indexed::TypedIndex,
    math::quadrangulation::{
        AnchorIndex, QuadEdge, QuadEdgeType, QuadError, QuadIndex, QuadVertex, Quadrangulation, Rot4Idx, VertexIndex,
    },
};
use shine_test::test;
use std::collections::HashSet;

/// 2x2 grid of 4 quads, 9 vertices, 1 interior vertex (4):
/// ```text
///  6----7----8
///  | Q2 | Q3 |
///  3----4----5
///  | Q0 | Q1 |
///  0----1----2
/// ```
/// Q0=[0,1,4,3]  Q1=[1,2,5,4]  Q2=[3,4,7,6]  Q3=[4,5,8,7]  (CCW)
/// Interior: 4.  Boundary: 8 vertices (0,1,2,5,8,7,6,3).
/// Simple 2x2 grid topology for testing.
fn grid_2x2() -> Quadrangulation {
    let quads = vec![
        [
            VertexIndex::new(0),
            VertexIndex::new(1),
            VertexIndex::new(4),
            VertexIndex::new(3),
        ],
        [
            VertexIndex::new(1),
            VertexIndex::new(2),
            VertexIndex::new(5),
            VertexIndex::new(4),
        ],
        [
            VertexIndex::new(3),
            VertexIndex::new(4),
            VertexIndex::new(7),
            VertexIndex::new(6),
        ],
        [
            VertexIndex::new(4),
            VertexIndex::new(5),
            VertexIndex::new(8),
            VertexIndex::new(7),
        ],
    ];
    let boundaries: Vec<_> = [0, 1, 2, 5, 8, 7, 6, 3].into_iter().map(VertexIndex::new).collect();
    let anchors: Vec<_> = [0, 2, 8, 6].into_iter().map(VertexIndex::new).collect();
    let positions = vec![
        Vec2::new(0.0, 0.0), // 0
        Vec2::new(1.0, 0.0), // 1
        Vec2::new(2.0, 0.0), // 2
        Vec2::new(0.0, 1.0), // 3
        Vec2::new(1.0, 1.0), // 4
        Vec2::new(2.0, 1.0), // 5
        Vec2::new(0.0, 2.0), // 6
        Vec2::new(1.0, 2.0), // 7
        Vec2::new(2.0, 2.0), // 8
    ];
    Quadrangulation::from_polygon(positions, boundaries, quads, anchors).expect("valid topology")
}

#[test]
fn test_quad_vertex_navigation() {
    // From local 0
    let qv0 = QuadVertex {
        quad: QuadIndex::new(0),
        local: Rot4Idx::new(0),
    };
    assert_eq!(qv0.next().local, Rot4Idx::new(1));
    assert_eq!(qv0.prev().local, Rot4Idx::new(3));
    assert_eq!(qv0.opposite().local, Rot4Idx::new(2));
    assert_eq!(qv0.outgoing_edge().edge, Rot4Idx::new(0));
    assert_eq!(qv0.incoming_edge().edge, Rot4Idx::new(3));

    // Wrapping: from local 3, next wraps to 0
    let qv3 = QuadVertex {
        quad: QuadIndex::new(0),
        local: Rot4Idx::new(3),
    };
    assert_eq!(qv3.next().local, Rot4Idx::new(0));
    assert_eq!(qv3.prev().local, Rot4Idx::new(2));
    assert_eq!(qv3.opposite().local, Rot4Idx::new(1));
    assert_eq!(qv3.outgoing_edge().edge, Rot4Idx::new(3));
    assert_eq!(qv3.incoming_edge().edge, Rot4Idx::new(2));
}

#[test]
fn test_quad_edge_navigation() {
    let qe1 = QuadEdge {
        quad: QuadIndex::new(0),
        edge: Rot4Idx::new(1),
    };
    assert_eq!(qe1.start().local, Rot4Idx::new(1));
    assert_eq!(qe1.end().local, Rot4Idx::new(2));

    // Wrapping: edge 3 ends at local 0
    let qe3 = QuadEdge {
        quad: QuadIndex::new(0),
        edge: Rot4Idx::new(3),
    };
    assert_eq!(qe3.start().local, Rot4Idx::new(3));
    assert_eq!(qe3.end().local, Rot4Idx::new(0));
}

#[test]
fn test_topology_counts() {
    let mesh = grid_2x2();

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
    let mesh = grid_2x2();

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
fn test_vertex_ring_ccw_interior() {
    let mesh = grid_2x2();

    // Interior vertex 4 ring should be [Q0, Q1, Q3, Q2] in some rotation
    let ring: Vec<_> = mesh.vertex_ring_ccw(VertexIndex::new(4)).map(|qv| qv.quad).collect();
    let expected = [
        QuadIndex::new(0),
        QuadIndex::new(1),
        QuadIndex::new(3),
        QuadIndex::new(2),
    ];
    assert!(
        is_rotation(&expected, &ring),
        "vertex 4 CCW ring {:?} should be a rotation of {:?}",
        ring,
        expected
    );
}

#[test]
fn test_vertex_ring_cw_interior() {
    let mesh = grid_2x2();

    // CW ring should be reverse rotation of CCW
    let ccw: Vec<_> = mesh.vertex_ring_ccw(VertexIndex::new(4)).map(|qv| qv.quad).collect();
    let cw: Vec<_> = mesh.vertex_ring_cw(VertexIndex::new(4)).map(|qv| qv.quad).collect();

    // Reverse CW should match some rotation of CCW
    let mut cw_reversed = cw.clone();
    cw_reversed.reverse();
    assert!(
        is_rotation(&ccw, &cw_reversed),
        "reversed CW ring {:?} should be a rotation of CCW ring {:?}",
        cw_reversed,
        ccw
    );
}

#[test]
fn test_vertex_ring_boundary_includes_infinite() {
    let mesh = grid_2x2();

    // Boundary vertex 0: its ring should include at least one infinite quad
    let ring: Vec<_> = mesh.vertex_ring_ccw(VertexIndex::new(0)).collect();
    let has_infinite = ring.iter().any(|qv| mesh.is_infinite_quad(qv.quad));
    assert!(has_infinite, "boundary vertex 0 ring should include an infinite quad");

    // Interior vertex 4: its ring should NOT include any infinite quad
    let ring: Vec<_> = mesh.vertex_ring_ccw(VertexIndex::new(4)).collect();
    let has_infinite = ring.iter().any(|qv| mesh.is_infinite_quad(qv.quad));
    assert!(
        !has_infinite,
        "interior vertex 4 ring should not include an infinite quad"
    );
}

#[test]
fn test_vertex_rings_consistency() {
    let mesh = grid_2x2();

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
fn test_boundary_detection() {
    let mesh = grid_2x2();

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
fn test_boundary_vertices_ccw_order() {
    let mesh = grid_2x2();

    let boundary: Vec<_> = mesh.boundary_vertices().collect();
    assert_eq!(boundary.len(), 8, "should have 8 boundary vertices");

    // All boundary vertices should be unique
    let mut seen = HashSet::new();
    for v in &boundary {
        assert!(seen.insert(*v), "boundary vertex {:?} appears multiple times", v);
    }

    let expected: Vec<_> = [0usize, 1, 2, 5, 8, 7, 6, 3]
        .into_iter()
        .map(VertexIndex::new)
        .collect();
    assert!(
        is_rotation(&expected, &boundary),
        "boundary {:?} should be a rotation of [0, 1, 2, 5, 8, 7, 6, 3]",
        boundary.iter().map(|v| v.into_index()).collect::<Vec<_>>()
    );
}

#[test]
fn test_edge_classification_interior() {
    let mesh = grid_2x2();

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
    let mesh = grid_2x2();

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
    let mesh = grid_2x2();

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
fn test_quad_neighbor_consistency() {
    let mesh = grid_2x2();

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
fn test_anchor_edges_ccw_order() {
    let mesh = grid_2x2();

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
fn test_topology_validation() {
    let mesh = grid_2x2();
    mesh.validate().expect("valid topology should pass validation");
}

#[test]
fn test_validation_rejects_odd_boundary() {
    let quads = vec![[
        VertexIndex::new(0),
        VertexIndex::new(1),
        VertexIndex::new(2),
        VertexIndex::new(3),
    ]];
    let boundary: Vec<_> = [0, 1, 2].into_iter().map(VertexIndex::new).collect();
    let positions = vec![Vec2::ZERO; 4];
    assert!(
        Quadrangulation::from_polygon(positions, boundary, quads, vec![]).is_err(),
        "odd boundary should be rejected"
    );
}

#[test]
fn test_validation_rejects_boundary_vertex_out_of_range() {
    let quads = vec![[
        VertexIndex::new(0),
        VertexIndex::new(1),
        VertexIndex::new(2),
        VertexIndex::new(3),
    ]];
    let boundary: Vec<_> = [0, 1, 99, 3].into_iter().map(VertexIndex::new).collect();
    let positions = vec![Vec2::ZERO; 4];
    match Quadrangulation::from_polygon(positions, boundary, quads, vec![]) {
        Err(QuadError::BoundaryVertexOutOfRange { vertex: 99, .. }) => {}
        Err(e) => panic!("expected BoundaryVertexOutOfRange, got: {}", e),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn test_validation_rejects_duplicate_boundary_vertex() {
    let quads = vec![[
        VertexIndex::new(0),
        VertexIndex::new(1),
        VertexIndex::new(2),
        VertexIndex::new(3),
    ]];
    let boundary: Vec<_> = [0, 1, 0, 3].into_iter().map(VertexIndex::new).collect();
    let positions = vec![Vec2::ZERO; 4];
    assert!(
        Quadrangulation::from_polygon(positions, boundary, quads, vec![]).is_err(),
        "duplicate boundary vertex should be rejected"
    );
}

#[test]
fn test_validation_rejects_quad_vertex_out_of_range() {
    let quads = vec![[
        VertexIndex::new(0),
        VertexIndex::new(1),
        VertexIndex::new(99),
        VertexIndex::new(3),
    ]];
    let boundary: Vec<_> = [0, 1, 2, 3].into_iter().map(VertexIndex::new).collect();
    let positions = vec![Vec2::ZERO; 4];
    match Quadrangulation::from_polygon(positions, boundary, quads, vec![]) {
        Err(QuadError::QuadVertexOutOfRange { vertex: 99, .. }) => {}
        Err(e) => panic!("expected QuadVertexOutOfRange, got: {}", e),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn test_average_adjacent_positions_interior() {
    let mesh = grid_2x2();

    // Interior vertex 4 at (1,1) has neighbors 1,3,5,7
    // Avg = ((1,0) + (0,1) + (2,1) + (1,2)) / 4 = (4,4)/4 = (1,1)
    let avg = mesh.average_adjacent_positions(VertexIndex::new(4));
    assert!((avg.x - 1.0).abs() < 0.001, "avg.x should be 1.0, got {}", avg.x);
    assert!((avg.y - 1.0).abs() < 0.001, "avg.y should be 1.0, got {}", avg.y);
}

#[test]
fn test_average_adjacent_positions_boundary() {
    let mesh = grid_2x2();

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
    let mesh = grid_2x2();

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
    let mesh = grid_2x2();

    assert_eq!(mesh.finite_quad_count(), 4);
}
