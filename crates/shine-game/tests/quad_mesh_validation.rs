use glam::Vec2;
use shine_game::{
    indexed::TypedIndex,
    math::quadrangulation::{QuadError, Quadrangulation, VertexIndex},
};
use shine_test::test;

fn expect_err(result: Result<Quadrangulation, QuadError>) -> QuadError {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected Err, got Ok"),
    }
}

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
    assert_eq!(
        expect_err(Quadrangulation::from_polygon(positions, boundary, quads, vec![])),
        QuadError::Input("Boundary must have even length, got 3".to_string())
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
    assert_eq!(
        expect_err(Quadrangulation::from_polygon(positions, boundary, quads, vec![])),
        QuadError::Input("Boundary vertex 99 >= vertex_count 4".to_string())
    );
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
    assert_eq!(
        expect_err(Quadrangulation::from_polygon(positions, boundary, quads, vec![])),
        QuadError::Input("Duplicate edge (0, 1) appears more than twice in quads".to_string())
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
    assert_eq!(
        expect_err(Quadrangulation::from_polygon(positions, boundary, quads, vec![])),
        QuadError::Input("Quad vertex 99 >= vertex_count 4".to_string())
    );
}
