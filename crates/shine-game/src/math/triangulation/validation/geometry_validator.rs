use crate::{
    indexed::TypedIndex,
    math::triangulation::{
        predicates::{in_circle, orient2d},
        Rot3Idx, Validator, VertexClue,
    },
};
use log::trace;

impl<'a, const DELAUNAY: bool> Validator<'a, DELAUNAY> {
    pub fn validate_geometry(&self) -> Result<(), String> {
        self.validate_duplicate_positions()?;
        self.validate_orientation()?;
        self.validate_area()?;
        if DELAUNAY {
            self.validate_delaunay()?;
        }
        Ok(())
    }

    pub fn validate_duplicate_positions(&self) -> Result<(), String> {
        let tri = self.tri;
        if tri.dimension() < 1 {
            return Ok(());
        }

        let mut positions: Vec<_> = tri
            .vertex_index_iter()
            .filter(|&v| !tri.is_infinite_vertex(v))
            .map(|v| (tri[v].position, v))
            .collect();
        positions.sort_by(|a, b| a.0.x.cmp(&b.0.x).then(a.0.y.cmp(&b.0.y)));
        for w in positions.windows(2) {
            if w[0].0 == w[1].0 {
                return Err(format!(
                    "Duplicate vertex positions: {:?} and {:?} both at ({}, {})",
                    w[0].1, w[1].1, w[0].0.x, w[0].0.y
                ));
            }
        }

        Ok(())
    }

    pub fn validate_orientation(&self) -> Result<(), String> {
        let tri = self.tri;
        if tri.dimension() < 2 {
            return Ok(());
        }

        for f in tri.face_index_iter() {
            if tri.is_infinite_face(f) {
                continue;
            }

            let p0 = tri.p(VertexClue::face_vertex(f, Rot3Idx::new(0)));
            let p1 = tri.p(VertexClue::face_vertex(f, Rot3Idx::new(1)));
            let p2 = tri.p(VertexClue::face_vertex(f, Rot3Idx::new(2)));

            if orient2d(p0, p1, p2) <= 0 {
                return Err(format!("Count-clockwise property is violated for {:?}", f));
            }
        }

        Ok(())
    }

    pub fn validate_area(&self) -> Result<(), String> {
        let tri = self.tri;
        if tri.dimension() != 2 {
            return Ok(());
        }

        // calculate the area of the triangles
        let mut tri_area = 0;
        for f in tri.face_index_iter() {
            if tri.is_infinite_face(f) {
                continue;
            }

            let a = tri[VertexClue::face_vertex(f, Rot3Idx::new(0))].position;
            let b = tri[VertexClue::face_vertex(f, Rot3Idx::new(1))].position;
            let c = tri[VertexClue::face_vertex(f, Rot3Idx::new(2))].position;

            let ax: i64 = a.x as i64;
            let ay: i64 = a.y as i64;
            let bx: i64 = b.x as i64;
            let by: i64 = b.y as i64;
            let cx: i64 = c.x as i64;
            let cy: i64 = c.y as i64;
            let abx = bx - ax;
            let aby = by - ay;
            let acx = cx - ax;
            let acy = cy - ay;
            tri_area += abx * acy - aby * acx; // twice the area of the triangle
        }

        // calculate the area of the convex hull
        let mut convex_area = 0;
        let end = tri.infinite_face();
        let mut cur = end;
        loop {
            let iid = tri[cur].find_vertex(tri.infinite_vertex()).unwrap(); // index of infinite vertex
            let aid = iid.decrement();
            let bid = iid.increment();
            let a = tri[VertexClue::face_vertex(cur, aid)].position;
            let b = tri[VertexClue::face_vertex(cur, bid)].position;
            let ax: i64 = a.x as i64;
            let ay: i64 = a.y as i64;
            let bx: i64 = b.x as i64;
            let by: i64 = b.y as i64;

            convex_area += ax * by - bx * ay;
            cur = tri[cur].neighbors[aid];
            if cur == end {
                break;
            }
        }

        trace!(
            "tri_area={}, convex_area={}, area_diff={}",
            tri_area,
            convex_area,
            convex_area - tri_area
        );

        if convex_area != tri_area {
            Err(format!(
                "Area of convex hull differs from polygon too much: tri_area={}, convex_area={}, area_diff={}",
                tri_area,
                convex_area,
                convex_area - tri_area
            ))
        } else {
            Ok(())
        }
    }

    pub fn validate_delaunay(&self) -> Result<(), String> {
        let tri = self.tri;
        if tri.dimension() != 2 {
            return Ok(());
        }

        for f in tri.face_index_iter() {
            if tri.is_infinite_face(f) {
                continue;
            }

            for i in 0..=2 {
                let edge = Rot3Idx::new(i);

                // Skip constrained edges - they're allowed to violate Delaunay
                if tri.c((f, edge)) != 0 {
                    continue;
                }

                let nf = tri[f].neighbors[edge];

                // Skip edges adjacent to infinite faces
                if tri.is_infinite_face(nf) {
                    continue;
                }

                let ni = tri[nf].find_neighbor(f).unwrap();

                // Get the 4 vertices of the quad:
                //   va = opposite vertex in face f
                //   vb, vc = shared edge endpoints
                //   vd = opposite vertex in neighbor face nf
                let va = tri[f].vertices[edge];
                let vb = tri[f].vertices[edge.increment()];
                let vc = tri[f].vertices[edge.decrement()];
                let vd = tri[nf].vertices[ni];

                let pa = tri[va].position;
                let pb = tri[vb].position;
                let pc = tri[vc].position;
                let pd = tri[vd].position;

                // Check if vd is strictly inside the circumcircle of CCW triangle (va, vb, vc)
                if in_circle(pa, pb, pc, pd) > 0 {
                    return Err(format!(
                        "Delaunay property violated: vertex {:?} at ({},{}) is inside circumcircle of face {:?} with vertices {:?}({},{}), {:?}({},{}), {:?}({},{})",
                        vd, pd.x, pd.y,
                        f,
                        va, pa.x, pa.y,
                        vb, pb.x, pb.y,
                        vc, pc.x, pc.y
                    ));
                }
            }
        }

        Ok(())
    }
}
