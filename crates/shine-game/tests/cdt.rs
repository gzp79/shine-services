use glam::IVec2;
use shine_game::math::cdt::{CdtError, Triangulation};

fn iv(x: i32, y: i32) -> IVec2 {
    IVec2::new(x, y)
}

#[test]
fn fuzz_crash_dupes_only() {
    // 3 points but only 2 unique — should return error, not panic
    let points = vec![iv(-1, -1), iv(-1, -1), iv(0, 0)];
    let _ = Triangulation::build(&points);
}

#[test]
fn simple_triangle() {
    let pts = [iv(0, 0), iv(1000, 0), iv(1000, 1000), iv(0, 1000)];
    let t = Triangulation::build(&pts[0..]).unwrap();
    assert!(t.inside(iv(500, 500)));
}

#[test]
fn duplicate_point() {
    let points = vec![iv(0, 0), iv(1000, 0), iv(1100, 1100), iv(1100, 1100), iv(0, 1000)];
    let edges = vec![(0, 1), (1, 2), (3, 4), (4, 0)];
    let t = Triangulation::build_with_edges(&points, &edges);
    assert!(!t.is_err());
    assert!(t.unwrap().inside(iv(500, 500)));
}

#[test]
fn simple_polygon() {
    let r = 10000i32;
    let mut points = Vec::new();
    let mut edges = Vec::new();
    const N: usize = 8;
    for i in 0..N {
        let a = (i as f64) / (N as f64) * std::f64::consts::PI * 2.0;
        let x = (a.cos() * r as f64).round() as i32;
        let y = (a.sin() * r as f64).round() as i32;
        points.push(iv(x, y));
        edges.push((i, (i + 1) % N));
    }
    let t = Triangulation::build_with_edges(&points, &edges).unwrap();
    assert!(t.inside(iv(0, 0)));
    assert!(!t.inside(iv(r + 100, 0)));
}

#[test]
fn simple_circle() {
    let r = 10000i32;
    let mut edges = Vec::new();
    let mut points = Vec::new();
    const N: usize = 22;
    for i in 0..N {
        let a = (i as f64) / (N as f64) * std::f64::consts::PI * 2.0;
        let x = (a.cos() * r as f64).round() as i32;
        let y = (a.sin() * r as f64).round() as i32;
        points.push(iv(x, y));
        edges.push((i, (i + 1) % N));
    }
    let t = Triangulation::build_with_edges(&points, &edges).unwrap();
    assert!(t.inside(iv(0, 0)));
    assert!(!t.inside(iv(r + 100, 0)));
}

#[test]
fn dupe_start() {
    let points = vec![
        iv(500, 500),
        iv(500, 500),
        iv(500, 600),
        iv(600, 500),
        iv(500, 400),
        iv(0, 0),
        iv(1000, 0),
        iv(1000, 1000),
        iv(0, 1000),
    ];
    let edges = vec![(1, 2), (2, 3), (3, 4), (4, 0)];
    let t = Triangulation::build_with_edges(&points, &edges).unwrap();
    assert!(t.inside(iv(550, 500)));
    assert!(!t.inside(iv(450, 500)));
}

#[test]
fn colinear_start() {
    let points = vec![
        iv(0, 0),
        iv(1000, 0),
        iv(1000, 1000),
        iv(0, 1000),
        iv(500, 400),
        iv(500, 500),
        iv(500, 600),
        iv(600, 500),
    ];
    let edges = vec![(4, 5), (5, 6), (6, 7), (7, 4)];
    let t = Triangulation::build_with_edges(&points, &edges).unwrap();
    assert!(t.inside(iv(550, 500)));
    assert!(!t.inside(iv(450, 500)));
}

#[test]
fn spiral_circle() {
    let r = 10000i32;
    let mut edges = Vec::new();
    let mut points = Vec::new();
    const N: usize = 16;
    for i in 0..N {
        let a = (i as f64) / (N as f64) * std::f64::consts::PI * 2.0;
        let x = (a.cos() * r as f64).round() as i32;
        let y = (a.sin() * r as f64).round() as i32;
        points.push(iv(x, y));
        edges.push((i, (i + 1) % N));
    }
    const M: usize = 32;
    for i in 0..(2 * M) {
        let a = (i as f64) / (M as f64) * std::f64::consts::PI * 2.0;
        let scale = (i as f64 + 1.1).powf(0.2);
        let x = (a.cos() * r as f64 / scale).round() as i32;
        let y = (a.sin() * r as f64 / scale).round() as i32;
        points.push(iv(x, y));
    }

    let t = Triangulation::build_with_edges(&points, &edges).unwrap();
    assert!(t.inside(iv(0, 0)));
    assert!(!t.inside(iv(r + 100, 0)));
}

#[test]
fn nested_circles() {
    let r_outer = 10000i32;
    let r_inner = 5000i32;
    let mut edges = Vec::new();
    let mut points = Vec::new();
    const N: usize = 32;
    for i in 0..N {
        let a = (i as f64) / (N as f64) * std::f64::consts::PI * 2.0;
        let x = (a.cos() * r_outer as f64).round() as i32;
        let y = (a.sin() * r_outer as f64).round() as i32;
        points.push(iv(x, y));
        edges.push((i, (i + 1) % N));
    }
    for i in 0..N {
        let a = (i as f64) / (N as f64) * std::f64::consts::PI * 2.0;
        let x = (a.cos() * r_inner as f64).round() as i32;
        let y = (a.sin() * r_inner as f64).round() as i32;
        points.push(iv(x, y));
        edges.push((N + i, N + (i + 1) % N));
    }

    let t = Triangulation::build_with_edges(&points, &edges).unwrap();
    assert!(!t.inside(iv(0, 0)));
    assert!(!t.inside(iv(r_outer + 100, 0)));
    assert!(t.inside(iv(7500, 0)));
    assert!(t.inside(iv(0, 8000)));
}

#[test]
fn grid() {
    let mut points = Vec::new();
    const N: usize = 32;
    for i in 0..N {
        for j in 0..N {
            points.push(iv(i as i32 * 100, j as i32 * 100));
        }
    }
    let t = Triangulation::build(&points).unwrap();
    t.check();
}

#[test]
fn grid_with_fixed_circle() {
    let r = 9000i32;
    let mut edges = Vec::new();
    let mut points = Vec::new();
    const N: usize = 32;
    for i in 0..N {
        let a = (i as f64) / (N as f64) * std::f64::consts::PI * 2.0;
        let x = (a.cos() * r as f64).round() as i32;
        let y = (a.sin() * r as f64).round() as i32;
        points.push(iv(x, y));
        edges.push((i, (i + 1) % N));
    }
    const M: usize = 32;
    for i in 0..M {
        for j in 0..M {
            let x = (i as i32 * 20000 / M as i32) - 10000;
            let y = (j as i32 * 20000 / M as i32) - 10000;
            points.push(iv(x, y));
        }
    }
    let t = Triangulation::build_with_edges(&points, &edges).unwrap();
    t.check();
}

#[test]
fn new_from_contours() {
    let t = Triangulation::build_from_contours::<Vec<usize>>(&[iv(0, 0), iv(1000, 0), iv(1000, 1000)], &vec![]);
    assert!(t.is_ok());

    let t = Triangulation::build_from_contours(&[iv(0, 0), iv(1000, 0), iv(1000, 1000)], &[vec![]]);
    assert!(t.is_ok());

    let t = Triangulation::build_from_contours(&[iv(0, 0), iv(1000, 0), iv(1000, 1000)], &[vec![0]]);
    assert!(t.is_ok());

    let t = Triangulation::build_from_contours(&[iv(0, 0), iv(1000, 0), iv(1000, 1000)], &[vec![0, 1]]);
    assert!(t.is_err());
    if let Err(e) = t {
        assert!(e == CdtError::OpenContour);
    }
}

/// Fuzz test: random constraints that may cross without intersection points as vertices.
#[test]
fn fuzz_crossing_constraints() {
    use rand::prelude::*;
    use std::collections::HashSet;

    let mut rng = rand::rng();

    for trial in 0..200 {
        let n_pts: usize = rng.random_range(10..30);
        let bound = 5000i32;
        let mut points: Vec<IVec2> = Vec::with_capacity(n_pts);
        let mut seen = HashSet::new();
        while points.len() < n_pts {
            let x: i32 = rng.random_range(-bound..=bound);
            let y: i32 = rng.random_range(-bound..=bound);
            if seen.insert((x, y)) {
                points.push(iv(x, y));
            }
        }

        let n_edges: usize = rng.random_range(2..8);
        let mut edges = Vec::with_capacity(n_edges);
        for _ in 0..n_edges {
            let a: usize = rng.random_range(0..n_pts);
            let mut b: usize = rng.random_range(0..n_pts);
            if b == a {
                b = (a + 1) % n_pts;
            }
            edges.push((a, b));
        }

        let result = Triangulation::build_with_edges(&points, &edges);
        match &result {
            Ok(t) => {
                t.check();
            }
            Err(CdtError::CrossingFixedEdge) => {}
            Err(CdtError::PointOnFixedEdge(_)) => {}
            Err(e) => {
                panic!("trial {trial}: unexpected error: {e}");
            }
        }
    }
}
