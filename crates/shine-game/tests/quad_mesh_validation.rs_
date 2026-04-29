use glam::Vec2;
use shine_game::{
    indexed::TypedIndex,
    math::quadrangulation::{QuadError, Quadrangulation, VertexIndex},
};
use shine_test::test;

#[test]
fn test_topology_validation() {
    let mesh = Quadrangulation::new_2x2_grid();
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
