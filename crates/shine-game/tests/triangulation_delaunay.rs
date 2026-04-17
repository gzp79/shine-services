use glam::IVec2;
use shine_game::math::triangulation::{debug, Triangulation};
use shine_test::test;

#[test]
fn cdt_simple() {
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
fn cdt_concave() {
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

#[test]
fn cdt_issue1() {
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
fn cdt_issue2() {
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
fn cdt_issue3() {
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
fn cdt_issue4() {
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
fn cdt_issue4_reduced() {
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
    let mut builder = tri.builder().with_debug(0, "../../temp/cdt/cdt_issue4_reduced");

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
fn cdt_issue5() {
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
    let mut builder = tri.builder().with_debug(0, "../../temp/cdt/cdt_issue5");

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
fn cdt_issue6() {
    let points: Vec<(i32, i32)> = vec![
        (20397, -21061),
        (20411, 9659),
        (21845, 21845),
        (1365, -1),
        (-1, -17542),
        (-1, 20303),
        (-177, -1),
        (20479, 20303),
        (20303, 20303),
        (20303, 20303),
        (23551, -1),
        (-1, 255),
    ];

    let points: Vec<_> = points.into_iter().map(|(x, y)| IVec2::new(x, y)).collect();

    let (points, _) = debug::reduce_test_case(points, vec![], |pnts, _| {
        let mut tri = Triangulation::new_cdt();
        let mut builder = tri.builder().with_debug(usize::MAX, "../../temp/cdt/cdt_issue6");
        builder.add_points(pnts.iter().cloned());
        builder.check().is_err()
    });

    let mut tri = Triangulation::new_cdt();
    let mut builder = tri.builder().with_debug(usize::MAX, "../../temp/cdt/cdt_issue6");
    builder.add_points(points);
    assert_eq!(builder.check(), Ok(()));
}

#[test]
fn cdt_issue6_reduced() {
    let points: Vec<IVec2> = vec![(-1, -175), (-1, 203), (-17, -1), (235, -1), (-1, 25)]
        .into_iter()
        .map(|(x, y)| IVec2::new(x, y))
        .collect();

    let mut tri = Triangulation::new_cdt();
    let mut builder = tri
        .builder()
        .with_debug(usize::MAX, "../../temp/cdt/cdt_issue6_reduced");
    builder.add_points(points);
    assert_eq!(builder.check(), Ok(()));
}
