use glam::IVec2;
use shine_game::math::triangulation::Triangulation;
use shine_test::test;

#[test]
fn cdt_test() {
    let mut tri = Triangulation::new_cdt();
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

    builder.add_constraint_edge(v00, v22, 1);
    builder.add_constraint_edge(v20, v02, 2);
    builder.add_constraint_edge(v10, v12, 4);
    builder.add_constraint_edge(v01, v21, 8);
    assert_eq!(builder.check(), Ok(()));
}

#[test]
fn cdt_constraint_concave() {
    let transforms: Vec<(&str, Box<dyn Fn(i32, i32) -> IVec2>)> = vec![
        ("(x, y)", Box::new(|x, y| IVec2::new(x, y))),
        ("(-x, y)", Box::new(|x, y| IVec2::new(-x, y))),
        ("(-x, -y)", Box::new(|x, y| IVec2::new(-x, -y))),
        ("(x, -y)", Box::new(|x, y| IVec2::new(x, -y))),
        ("(y, x)", Box::new(|x, y| IVec2::new(y, x))),
        ("(-y, x)", Box::new(|x, y| IVec2::new(-y, x))),
        ("(-y, -x)", Box::new(|x, y| IVec2::new(-y, -x))),
        ("(y, -x)", Box::new(|x, y| IVec2::new(y, -x))),
    ];

    for (info, map) in transforms.iter() {
        log::debug!("transformation: {}", info);

        let mut tri = Triangulation::new_cdt();
        let mut builder = tri.builder();

        let _e = builder.add_vertex(map(20, 25), None);
        let _d = builder.add_vertex(map(35, 25), None);
        let _b = builder.add_vertex(map(20, 5), None);
        let _c = builder.add_vertex(map(35, 0), None);
        let _a = builder.add_vertex(map(10, 0), None);
        let p0 = builder.add_vertex(map(0, 10), None);
        let _f = builder.add_vertex(map(10, 15), None);
        let p1 = builder.add_vertex(map(40, 10), None);
        assert_eq!(builder.check(), Ok(()));

        builder.add_constraint_edge(p0, p1, 1);
        assert_eq!(builder.check(), Ok(()));
        assert_eq!(tri.c(tri.find_edge_by_vertex(p0, p1).unwrap()), 1);
    }
}
