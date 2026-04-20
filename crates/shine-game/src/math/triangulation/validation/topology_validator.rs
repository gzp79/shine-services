use crate::{
    indexed::TypedIndex,
    math::triangulation::{Rot3Idx, Validator},
};

impl<'a, const DELAUNAY: bool> Validator<'a, DELAUNAY> {
    pub fn validate_topology(&self) -> Result<(), String> {
        self.validate_dimension()?;
        self.validate_links()?;
        Ok(())
    }

    pub fn validate_dimension(&self) -> Result<(), String> {
        let tri = self.tri;

        if tri.dimension() == u8::MAX {
            if tri.vertex_count() != 0 {
                Err(format!("Empty triangulation has vertices: {}", tri.vertex_count()))
            } else if tri.face_count() != 0 {
                Err(format!("Empty triangulation has faces: {}", tri.face_count()))
            } else if tri.infinite_vertex().is_valid() {
                Err(format!(
                    "Empty triangulation has a valid infinite vertex: {:?}",
                    tri.infinite_vertex()
                ))
            } else {
                Ok(())
            }
        } else {
            let finite_vertex_count = tri.vertex_index_iter().filter(|&v| !tri.is_infinite_vertex(v)).count();
            if finite_vertex_count != tri.vertex_count() - 1 {
                return Err(format!(
                    "Number of finite vertices is invalid, got {}, expected: {}",
                    finite_vertex_count,
                    tri.vertex_count() - 1
                ));
            }

            let mut finite_face_count = 0;
            let mut infinite_face_count = 0;
            for f in tri.face_index_iter() {
                for r in 0..3 {
                    let d = Rot3Idx::new(r);
                    if tri[f].vertices[d].is_valid() != (r <= tri.dimension() as usize) {
                        return Err(format!(
                            "A face({:?}) has invalid dimension at {:?} (dim:{})",
                            f,
                            d,
                            tri.dimension()
                        ));
                    }
                }
                if tri.is_infinite_face(f) {
                    infinite_face_count += 1;
                } else {
                    finite_face_count += 1;
                }
            }

            if tri.dimension() == 0 {
                if finite_face_count != 1 {
                    Err(format!(
                        "Face count does not match for dim0, (f = 1), f={}",
                        finite_face_count
                    ))
                } else if infinite_face_count != 1 {
                    Err(format!(
                        "Infinit face count does not match for dim0: (h = 1), h={}",
                        infinite_face_count
                    ))
                } else if finite_vertex_count != 1 {
                    Err(format!(
                        "Vertex count does not match for dim0: (v = 1), v={}",
                        finite_vertex_count
                    ))
                } else {
                    Ok(())
                }
            } else if tri.dimension() == 1 {
                if infinite_face_count != 2 {
                    Err(format!(
                        "Infinite face count does not match hull count for dim1: (if = h), if={},h=2",
                        infinite_face_count
                    ))
                } else if finite_face_count + 1 != finite_vertex_count {
                    Err(format!(
                        "Vertex, face count does not match for dim1: (v = f+1), v={},f={}",
                        finite_vertex_count, finite_face_count
                    ))
                } else {
                    Ok(())
                }
            } else if tri.dimension() == 2 {
                let mut hull_count = 0;
                let end = tri.infinite_face();
                let mut cur = end;
                loop {
                    hull_count += 1;
                    let iid = tri[cur].find_vertex(tri.infinite_vertex()).unwrap(); // index of infinite vertex
                    let aid = iid.decrement();
                    cur = tri[cur].neighbors[aid];
                    if cur == end {
                        break;
                    }
                }

                if hull_count != infinite_face_count {
                    Err(format!(
                        "Infinite face count does not match hull count for dim2: (if = h), if={},h={}",
                        infinite_face_count, hull_count
                    ))
                } else if 2 * finite_vertex_count != finite_face_count + hull_count + 2 {
                    // https://en.wikipedia.org/wiki/Point_set_triangulation#Combinatorics_in_the_plane
                    Err(format!(
                        "Vertex, face count does not match for dim2: (2v-h-2 = f), v={},f={},h={}",
                        finite_vertex_count, finite_face_count, hull_count
                    ))
                } else {
                    Ok(())
                }
            } else {
                Err(format!("Invalid dimension: {}", tri.dimension()))
            }
        }
    }

    pub fn validate_links(&self) -> Result<(), String> {
        if self.tri.is_empty() {
            return Ok(());
        }

        self.validate_vertex_face_link()?;
        self.validate_face_face_link()?;

        Ok(())
    }

    fn validate_vertex_face_link(&self) -> Result<(), String> {
        let tri = self.tri;

        for v in tri.vertex_index_iter() {
            if !tri[v].face.is_valid() {
                return Err(format!("Vertex-face link is invalid, no face for {:?} ", v));
            }

            let nf = tri[v].face;
            let _vi = tri[nf]
                .find_vertex(v)
                .ok_or_else(|| format!("Vertex-face link is invalid {:?} is not a neighbor of {:?}", nf, v))?;
        }
        Ok(())
    }

    fn validate_face_face_link(&self) -> Result<(), String> {
        let tri = self.tri;

        for f in tri.face_index_iter() {
            for d in 0..=tri.dimension() {
                let i = Rot3Idx::new(d as usize);
                let nf = tri[f].neighbors[i];
                if !nf.is_valid() {
                    return Err(format!(
                        "Face-face link is invalid, no neighboring face for {:?} at {:?}",
                        f, i
                    ));
                }

                let ni = tri[nf].find_neighbor(f).ok_or_else(|| {
                    format!(
                        "Face-face link is invalid, missing backward link between ({:?},{:?}) and {:?}",
                        f, i, nf
                    )
                })?;

                match tri.dimension() {
                    1 => {
                        if tri[f].vertices[i.mirror(2)] != tri[nf].vertices[ni.mirror(2)] {
                            return Err(format!(
                                "Face-face link is invalid, vertex relation in dim1 ({:?},{:?}) <-> ({:?},{:?})",
                                f, i, nf, ni
                            ));
                        }
                        if tri[f].constraints[i] != tri[nf].constraints[ni] {
                            return Err(format!(
                                "Face-face link is invalid, non-matching constraints in dim1 ({:?},{:?}) <-> ({:?},{:?})",
                                f, d, nf, ni
                            ));
                        }
                    }
                    2 => {
                        if tri[f].vertices[i.decrement()] != tri[nf].vertices[ni.increment()]
                            || tri[f].vertices[i.increment()] != tri[nf].vertices[ni.decrement()]
                        {
                            return Err(format!(
                                "Face-face link is invalid, vertex relation in dim2 ({:?},{:?}) <-> ({:?},{:?})",
                                f, d, nf, ni
                            ));
                        }
                        if tri[f].constraints[i] != tri[nf].constraints[ni] {
                            return Err(format!(
                                "Face-face link is invalid, non-matching constraints in dim2 ({:?},{:?}) <-> ({:?},{:?})",
                                f, d, nf, ni
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
