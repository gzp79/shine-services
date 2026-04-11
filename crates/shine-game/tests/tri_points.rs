use glam::IVec2;
use shine_game::math::triangulation::{GeometryChecker, TopologyChecker, Triangulation};
use shine_test::test;

#[test]
fn triangulate_empty() {
    let tri = Triangulation::new_ct();
    assert!(tri.is_empty());
    assert_eq!(tri.dimension(), u8::MAX);
    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
}

#[test]
fn triangulate_point() {
    let mut tri = Triangulation::new_ct();

    log::trace!("add a point");
    let vi = { tri.builder().add_vertex(IVec2::new(10, 20), None) };
    assert!(!tri.is_empty());
    assert_eq!(tri.dimension(), 0);
    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

    log::trace!("add same point twice");
    let vi2 = { tri.builder().add_vertex(IVec2::new(10, 20), None) };
    assert_eq!(tri.dimension(), 0);
    assert_eq!(vi, vi2);
    assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
    assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

    log::trace!("clear");
    tri.clear();
    assert!(tri.is_empty());
    assert_eq!(tri.dimension(), u8::MAX);
}

#[test]
fn triangulate_line() {
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

        let positions = vec![0, 4, 2, 1, 3, 7];
        for (i, &p) in positions.iter().enumerate() {
            let expected_dim = match i {
                0 => 0,
                _ => 1,
            };

            let pos = map(p);
            log::trace!("add {:?}", pos);
            let vi = tri.builder().add_vertex(pos, None);
            assert_eq!(tri.dimension(), expected_dim);
            assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
            assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

            let pos = map(p);
            log::trace!("add duplicate {:?}", pos);
            let vi_dup = tri.builder().add_vertex(pos, None);
            assert_eq!(tri.dimension(), expected_dim);
            assert_eq!(vi, vi_dup);
            assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
            assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        }

        log::trace!("clear");
        tri.clear();
        assert!(tri.is_empty());
        assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
        assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
    }
}

#[test]
fn triangulate_plane() {
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
    #[rustfmt::skip]
    let test_cases = vec![
        vec![(0, 0), (20, 0), (10, 20)],
        vec![(0, 0), (10, 0), (20, 0), (10, 20)],
        vec![(0, 0), (5, 0), (10, 0), (15, 0), (20, 0), (10, 20)],
        vec![(0, 0), (20, 0), (10, 20), (0, 0), (5, 0), (10, 0), (15, 0), (20, 0), (10, 20)],
        vec![(0, 0), (20, 0), (15, 0), (10, 0), (5, 0), (10, 20)],
        vec![(0, 0), (15, 0), (10, 0), (5, 0), (20, 0), (10, 20)],
        vec![(0, 0), (20, 0), (10, 20), (10, 10)],
        vec![(0, 0), (20, 0), (10, 20), (30, 30)],
        vec![(0, 0), (20, 0), (10, 20), (30, -30)],
        vec![(0, 0), (20, 0), (10, 20), (-30, -30)],
        vec![(0, 0), (20, 0), (10, 20), (-30, 30)],
    ];

    for (info, map) in transforms.iter() {
        log::debug!("transformation: {}", info);

        for (i, pnts) in test_cases.iter().enumerate() {
            log::trace!("testcase: {}", i);

            for &(x, y) in pnts.iter() {
                let pos = map(x, y);
                log::trace!("add {:?}", pos);
                let vi = tri.builder().add_vertex(pos, None);
                log::trace!("{:?} = {:?}", vi, tri[vi].position);
                assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
                assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));

                let pos = map(x, y);
                log::trace!("add duplicate {:?}", pos);
                let vi_dup = tri.builder().add_vertex(pos, None);
                assert_eq!(vi, vi_dup);
                assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
                assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
            }

            assert_eq!(tri.dimension(), 2);

            log::trace!("clear");
            tri.clear();
            assert!(tri.is_empty());
            assert_eq!(TopologyChecker::new(&tri).check(), Ok(()));
            assert_eq!(GeometryChecker::new(&tri).check(), Ok(()));
        }
    }
}
