use glam::IVec2;
use shine_game::math::triangulation::{CrossingIterator, GeometryChecker, TopologyChecker, Triangulation};
use shine_test::test;

#[test]
fn t0_constraint_segment() {
    let transforms: Vec<(&str, Box<dyn Fn(i32) -> IVec2>)> = vec![
        ("(x, 0)", Box::new(|x| IVec2::new(x, 0))),
        ("(0, x)", Box::new(|x| IVec2::new(0, x))),
        ("(-x, 0)", Box::new(|x| IVec2::new(-x, 0))),
        ("(0, -x)", Box::new(|x| IVec2::new(0, -x))),
        ("(x, x)", Box::new(|x| IVec2::new(x, x))),
        ("(x, -x)", Box::new(|x| IVec2::new(x, -x))),
        ("(-x, -x)", Box::new(|x| IVec2::new(-x, -x))),
        ("(-x, x)", Box::new(|x| IVec2::new(-x, x))),
    ];

    let mut tri = Triangulation::new_ct();

    for (info, map) in transforms.iter() {
        log::debug!("transformation: {}", info);

        tri.builder().add_vertex(map(0), None);
        tri.builder().add_vertex(map(10), None);

        tri.builder().add_constraint_segment(map(2), map(5), 1);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        tri.builder().add_constraint_segment(map(3), map(7), 1);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        tri.builder().add_constraint_segment(map(8), map(1), 1);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

        log::trace!("clear");
        tri.clear();
        assert!(tri.is_empty());
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
    }
}

#[test]
fn t1_constraint_no_fill1() {
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

    let mut tri = Triangulation::new_ct();

    for (info, map) in transforms.iter() {
        log::debug!("transformation: {}", info);

        tri.builder().add_vertex(map(0, 0), None);
        tri.builder().add_vertex(map(10, 0), None);
        tri.builder().add_vertex(map(10, 10), None);

        tri.builder().add_constraint_segment(map(0, 0), map(10, 0), 1);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        tri.builder().add_constraint_segment(map(0, 0), map(10, 10), 2);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        tri.builder().add_constraint_segment(map(10, 0), map(10, 10), 4);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

        tri.builder().add_vertex(map(2, 0), None);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        tri.builder().add_vertex(map(5, 0), None);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        tri.builder().add_vertex(map(3, 0), None);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

        log::trace!("clear");
        tri.clear();
        assert!(tri.is_empty());
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
    }
}

#[test]
fn t2_constraint_no_fill2() {
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

    let mut tri = Triangulation::new_ct();

    for (info, map) in transforms.iter() {
        log::debug!("transformation: {}", info);

        tri.builder().add_vertex(map(0, 0), None);
        tri.builder().add_vertex(map(100, 0), None);
        tri.builder().add_vertex(map(100, 100), None);

        let c0 = tri.builder().add_constraint_segment(map(100, 0), map(100, 100), 1);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        let c1 = tri.builder().add_constraint_segment(map(20, 0), map(50, 0), 2);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        let c2 = tri.builder().add_constraint_segment(map(30, 0), map(70, 0), 4);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        let c3 = tri.builder().add_constraint_segment(map(0, 0), map(100, 0), 8);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        let c4 = tri.builder().add_constraint_segment(map(100, 0), map(0, 0), 16);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        let c5 = tri.builder().add_constraint_segment(map(100, 100), map(0, 0), 32);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        let c6 = tri.builder().add_constraint_segment(map(10, 10), map(90, 90), 64);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        let c7 = tri.builder().add_constraint_segment(map(90, 90), map(10, 10), 128);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        let c8 = tri.builder().add_constraint_segment(map(80, 80), map(20, 20), 256);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

        let _v0 = tri.builder().add_vertex(map(2, 5), None);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        let _v1 = tri.builder().add_vertex(map(5, 2), None);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        let v2 = tri.builder().add_vertex(map(50, 50), None);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

        assert_eq!(c0.1, c5.0);
        assert_eq!(c6.1, c7.0);
        assert_eq!(c6.0, c7.1);
        assert_eq!(c3.0, c4.1);
        assert_eq!(c3.0, c5.1);
        assert_eq!(c0.0, c3.1);
        assert_eq!(c0.0, c4.0);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c0.1, c0.0).unwrap()), 1);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c0.0, c2.1).unwrap()), 24);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c2.1, c1.1).unwrap()), 28);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c1.1, c2.0).unwrap()), 30);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c2.0, c1.0).unwrap()), 26);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c1.0, c3.0).unwrap()), 24);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c3.0, c6.0).unwrap()), 32);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c6.0, c8.1).unwrap()), 224);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c8.1, v2).unwrap()), 480);
        assert_eq!(tri.c(tri.find_edge_by_vertex(v2, c8.0).unwrap()), 480);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c8.0, c7.0).unwrap()), 224);
        assert_eq!(tri.c(tri.find_edge_by_vertex(c7.0, c0.1).unwrap()), 32);

        log::trace!("clear");
        tri.clear();
        assert!(tri.is_empty());
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
    }
}

#[test]
fn t3_crossing_iterator() {
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

    let mut tri = Triangulation::new_ct();

    for (info, map) in transforms.iter() {
        log::debug!("transformation: {}", info);

        let v1 = tri.builder().add_vertex(map(20, 10), None);
        let v2 = tri.builder().add_vertex(map(40, 10), None);
        let _3 = tri.builder().add_vertex(map(10, 20), None);
        let _4 = tri.builder().add_vertex(map(10, 0), None);
        let v5 = tri.builder().add_vertex(map(0, 10), None);
        let _6 = tri.builder().add_vertex(map(50, 20), None);
        let _7 = tri.builder().add_vertex(map(50, 0), None);
        let v8 = tri.builder().add_vertex(map(60, 10), None);
        let _ = tri.builder().add_vertex(map(5, 12), None);
        let _ = tri.builder().add_vertex(map(5, 8), None);
        let _ = tri.builder().add_vertex(map(8, 10), None);
        let _ = tri.builder().add_vertex(map(30, 10), None);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

        let crossing: Vec<_> = tri.crossing_iterator(v1, v2).take(10).collect();
        assert_eq!(crossing.len(), 2, "{:?}", crossing);

        let crossing: Vec<_> = tri.crossing_iterator(v2, v1).take(10).collect();
        assert_eq!(crossing.len(), 2, "{:?}", crossing);

        let crossing: Vec<_> = tri.crossing_iterator(v5, v2).take(10).collect();
        assert_eq!(crossing.len(), 7, "{:?}", crossing);

        let crossing: Vec<_> = tri.crossing_iterator(v2, v5).take(10).collect();
        assert_eq!(crossing.len(), 7, "{:?}", crossing);

        let crossing: Vec<_> = tri.crossing_iterator(v5, v8).take(20).collect();
        assert_eq!(crossing.len(), 9, "{:?}", crossing);

        let crossing: Vec<_> = tri.crossing_iterator(v8, v5).take(20).collect();
        assert_eq!(crossing.len(), 9, "{:?}", crossing);

        log::trace!("clear");
        tri.clear();
        assert!(tri.is_empty());
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
    }
}

#[test]
fn t4_constraint_concave() {
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

    let mut tri = Triangulation::new_ct();

    for (info, map) in transforms.iter() {
        log::debug!("transformation: {}", info);

        let _e = tri.builder().add_vertex(map(20, 25), None);
        let _d = tri.builder().add_vertex(map(35, 25), None);
        let _b = tri.builder().add_vertex(map(20, 5), None);
        let _c = tri.builder().add_vertex(map(35, 0), None);
        let _a = tri.builder().add_vertex(map(10, 0), None);
        let p0 = tri.builder().add_vertex(map(0, 10), None);
        let _f = tri.builder().add_vertex(map(10, 15), None);
        let p1 = tri.builder().add_vertex(map(40, 10), None);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

        tri.builder().add_constraint_edge(p0, p1, 1);
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        assert_eq!(tri.c(tri.find_edge_by_vertex(p0, p1).unwrap()), 1);

        log::trace!("clear");
        tri.clear();
        assert!(tri.is_empty());
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
    }
}

#[test]
fn t5_constraint() {
    // coordinates multiplied by 10 to convert from float to integer
    let cases: Vec<(Vec<(i32, i32)>, Vec<(Vec<(usize, usize)>, Option<Vec<(usize, usize)>>)>)> = vec![
        (
            vec![(0, 0), (0, 10), (10, 0)],
            vec![
                (vec![(0, 1), (0, 2), (1, 2)], None),
                (vec![(0, 2), (0, 1), (1, 2)], None),
                (vec![(1, 2), (0, 1), (0, 2)], None),
            ],
        ),
        (
            vec![(-10, 0), (10, 0), (0, 30), (0, 20), (-20, 10), (20, 10)],
            vec![(vec![(4, 5)], None), (vec![(5, 4)], None)],
        ),
        (
            vec![
                (-100, 15),
                (-90, 25),
                (-80, 37),
                (-70, 20),
                (-60, 40),
                (-50, 70),
                (-40, 60),
                (-30, 80),
                (0, 30),
                (10, 50),
                (20, 10),
                (30, 90),
                (40, 40),
                (50, 60),
                (60, 20),
                (70, 80),
                (80, 90),
                (90, 50),
                (100, 70),
            ],
            vec![(
                vec![(3, 14), (12, 4), (6, 13), (18, 5), (15, 7), (9, 17), (11, 16)],
                None,
            )],
        ),
        (
            vec![(10, 20), (20, 10), (11, 10), (32, 50), (230, 30), (30, 100)],
            vec![(
                vec![(1, 2), (2, 0), (0, 1), (1, 4), (3, 4), (5, 0), (4, 5), (5, 1), (3, 5)],
                None,
            )],
        ),
        (
            vec![
                (20, 10),
                (40, 10),
                (10, 20),
                (10, 0),
                (0, 10),
                (50, 20),
                (50, 0),
                (60, 10),
                (5, 12),
                (5, 8),
                (8, 10),
                (30, 10),
            ],
            vec![(vec![(4, 7)], Some(vec![(4, 10), (10, 0), (0, 11), (11, 1), (1, 7)]))],
        ),
    ];

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

    let mut tri = Triangulation::new_ct();

    for (id_points, (points, edges)) in cases.iter().enumerate() {
        for (id_edges, (edges, edges_check)) in edges.iter().enumerate() {
            for (info, map) in transforms.iter() {
                log::debug!("{}/{}- transformation: {}", id_points, id_edges, info);

                let mut vertices = Vec::new();
                for v in points.iter() {
                    vertices.push(tri.builder().add_vertex(map(v.0, v.1), None));
                }
                assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
                assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

                for e in edges.iter() {
                    tri.builder().add_constraint_edge(vertices[e.0], vertices[e.1], 1);
                }
                assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
                assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

                let edges_check = edges_check.as_ref().unwrap_or(edges);
                for e in edges_check.iter() {
                    let edge = tri.find_edge_by_vertex(vertices[e.0], vertices[e.1]).expect(&format!(
                        "Missing edge between {:?} and {:?}",
                        vertices[e.0], vertices[e.1]
                    ));
                    assert_eq!(tri.c(edge), 1);
                }

                log::trace!("clear");
                tri.clear();
                assert!(tri.is_empty());
                assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
                assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
            }
        }
    }
}
