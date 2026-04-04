use glam::Vec2;
use shine_game::indexed::TypedIndex;
use shine_game::math::mesh::{QuadMesh, VertIdx};

#[test]
fn test_quad_centers_computed() {
    // Simple 2x2 quad mesh
    let positions = vec![
        Vec2::new(0.0, 0.0), // 0
        Vec2::new(1.0, 0.0), // 1
        Vec2::new(2.0, 0.0), // 2
        Vec2::new(0.0, 1.0), // 3
        Vec2::new(1.0, 1.0), // 4
        Vec2::new(2.0, 1.0), // 5
    ];

    let polygon = vec![
        VertIdx::new(0),
        VertIdx::new(1),
        VertIdx::new(2),
        VertIdx::new(5),
        VertIdx::new(4),
        VertIdx::new(3),
    ];

    let quads = vec![
        [VertIdx::new(0), VertIdx::new(1), VertIdx::new(4), VertIdx::new(3)],
        [VertIdx::new(1), VertIdx::new(2), VertIdx::new(5), VertIdx::new(4)],
    ];

    let mesh = QuadMesh::from_polygon(positions, polygon, quads).unwrap();

    assert_eq!(mesh.quad_centers.len(), 2);

    // First quad center: average of (0,0), (1,0), (1,1), (0,1) = (0.5, 0.5)
    let center0 = mesh.quad_centers[shine_game::math::mesh::QuadIdx::new(0)];
    assert!((center0.x - 0.5).abs() < 0.001);
    assert!((center0.y - 0.5).abs() < 0.001);

    // Second quad center: average of (1,0), (2,0), (2,1), (1,1) = (1.5, 0.5)
    let center1 = mesh.quad_centers[shine_game::math::mesh::QuadIdx::new(1)];
    assert!((center1.x - 1.5).abs() < 0.001);
    assert!((center1.y - 0.5).abs() < 0.001);
}

#[test]
fn test_quad_centers_count_matches_real_quads() {
    // Simple 1x1 quad mesh with one quad
    let positions = vec![
        Vec2::new(0.0, 0.0), // 0
        Vec2::new(1.0, 0.0), // 1
        Vec2::new(1.0, 1.0), // 2
        Vec2::new(0.0, 1.0), // 3
    ];

    let polygon = vec![VertIdx::new(0), VertIdx::new(1), VertIdx::new(2), VertIdx::new(3)];

    let quads = vec![[VertIdx::new(0), VertIdx::new(1), VertIdx::new(2), VertIdx::new(3)]];

    let mesh = QuadMesh::from_polygon(positions, polygon, quads).unwrap();

    assert_eq!(mesh.quad_centers.len(), mesh.topology.quad_count());
    assert_eq!(mesh.quad_centers.len(), 1);
}
