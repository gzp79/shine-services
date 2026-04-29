use crate::{
    indexed::TypedIndex,
    math::{
        debug::svg_content::{Content, ContentInfo},
        geometry::bisector,
        quadrangulation::{QuadEdge, QuadVertex, Quadrangulation, Rot4Idx},
        triangulation::{FaceEdge, Rot3Idx, Triangulation},
    },
};
use glam::{vec2, IVec2, Vec2};
use std::{collections::HashMap, io};

/// A conversion utility to visualizing Vec2-like types as Vec2 for SVG output.
pub trait DebugVec2 {
    fn to_vec2(self) -> Vec2;
}

impl DebugVec2 for Vec2 {
    fn to_vec2(self) -> Vec2 {
        self
    }
}

impl DebugVec2 for IVec2 {
    fn to_vec2(self) -> Vec2 {
        self.as_vec2()
    }
}

pub struct SvgDump {
    content: Vec<Content>,
    styles: HashMap<String, String>,
}

impl Default for SvgDump {
    fn default() -> Self {
        Self::new()
    }
}

impl SvgDump {
    pub fn new() -> Self {
        Self {
            content: Vec::new(),
            styles: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.styles.clear();
    }

    #[must_use]
    pub fn write(&self, stream: &mut dyn io::Write) -> io::Result<()> {
        let mut bounds = (Vec2::MAX, Vec2::MIN);
        for content in &self.content {
            let (min, max) = content.get_bound();
            bounds.0 = bounds.0.min(min);
            bounds.1 = bounds.1.max(max);
        }

        // Normalize to a fixed-size box while preserving aspect ratio
        const NORMALIZED_SIZE: f32 = 1000.0;
        let content_size = bounds.1 - bounds.0;
        let aspect = content_size.x / content_size.y;

        let (normalized_width, normalized_height) = if aspect > 1.0 {
            (NORMALIZED_SIZE, NORMALIZED_SIZE / aspect)
        } else {
            (NORMALIZED_SIZE * aspect, NORMALIZED_SIZE)
        };

        let scale = normalized_width / content_size.x;
        let normalized_bounds = (Vec2::ZERO, vec2(normalized_width, normalized_height));

        let info = ContentInfo {
            bounds,
            normalized_bounds,
            scale,
        };

        self.write_start(stream, normalized_bounds)?;
        self.write_style(stream)?;
        for content in &self.content {
            writeln!(stream, "{}", content.to_svg(&info))?;
        }
        self.write_end(stream)?;
        Ok(())
    }

    #[must_use]
    pub fn to_string(self) -> io::Result<String> {
        let mut output = Vec::new();
        self.write(&mut output)?;
        Ok(String::from_utf8(output).unwrap_or_default())
    }

    pub fn add_style<C: ToString, S: ToString>(&mut self, class: C, style: S) -> &mut Self {
        self.styles.insert(class.to_string(), style.to_string());
        self
    }

    pub fn add_content(&mut self, content: Content) -> &mut Self {
        self.content.push(content);
        self
    }

    #[rustfmt::skip]
    pub fn add_default_styles(&mut self) -> &mut Self {
        self.add_style("vert", "stroke: #2c3e50; stroke-width: 5; stroke-linecap: round; vector-effect: non-scaling-stroke;");
        self.add_style("vert-inf", "stroke: #e67e22; stroke-width: 5; stroke-linecap: round; vector-effect: non-scaling-stroke;");
        self.add_style("vert-text", "font-weight: bold; font-family: monospace; font-size: 20px; fill: #2c3e50;");

        self.add_style("edge", "stroke: #34495e; stroke-width: 1.5; stroke-linecap: round; vector-effect: non-scaling-stroke;");
        self.add_style("edge-constraint", "stroke: #27ae60; stroke-width: 1.5; stroke-linecap: round; vector-effect: non-scaling-stroke;");
        self.add_style("edge-neighbor", "stroke: #95a5a6; stroke-width: 1.5; opacity: 0.5; stroke-dasharray: 3,3; vector-effect: non-scaling-stroke;");
        self.add_style("edge-neighbor-error", "stroke: #950000; stroke-width: 1.5; opacity: 0.5; stroke-dasharray: 3,3; vector-effect: non-scaling-stroke;");
        self.add_style("edge-text", "font-family: monospace; font-size: 10px; fill: #7f8c8d; text-anchor: middle;");

        self.add_style("face-text", "font-style: italic; font-family: monospace; font-size: 20px; fill: #2c3e50; text-anchor: middle;");
        self
    }

    pub fn add_points_and_edges(
        &mut self,
        points: &[Vec2],
        edges: &[(usize, usize)],
        point_class: &str,
        text_class: &str,
        stroke_class: &str,
    ) -> &mut Self {
        for &(i0, i1) in edges {
            let p0 = points[i0];
            let p1 = points[i1];
            self.add_content(Content::Line {
                p0,
                p1,
                class: stroke_class.into(),
            });
        }

        for (i, &p) in points.iter().enumerate() {
            // Use zero-length line with round linecap for zoom-independent point
            self.add_content(Content::Point {
                pos: p,
                class: point_class.into(),
            });
            if text_class != "" {
                self.add_content(Content::Text {
                    pos: p,
                    offset: Vec2::ZERO,
                    text: i.to_string(),
                    class: text_class.to_string(),
                });
            }
        }

        self
    }

    pub fn add_polygon<V: DebugVec2, IV: Iterator<Item = V>>(&mut self, points: IV, edge_class: &str) -> &mut Self {
        let edge_class = if edge_class == "" { "edge" } else { edge_class };

        let mut prev: Option<Vec2> = None;
        for p in points {
            let p: Vec2 = p.to_vec2();
            if let Some(prev) = prev {
                self.add_content(Content::Line {
                    p0: prev,
                    p1: p,
                    class: edge_class.into(),
                });
            }
            prev = Some(p);
        }
        self
    }

    /// Add a triangulation to the SVG dump, with optional edge classifications for highlighting.
    /// The `edges` parameter is an iterator of tuples containing:
    /// - A list of `FaceEdge` references that identify edges in the triangulation.
    /// - A CSS class name to apply for highlighting those edges.
    /// - A boolean indicating whether to use the edge or the neightbor edge for highlighting (false = edge, true = neighbor).
    pub fn add_tri<'a, ECI: IntoIterator<Item = (&'a [FaceEdge], &'a str, bool)>, const D: bool>(
        &mut self,
        tri: &Triangulation<D>,
        edges: ECI,
    ) -> &mut Self {
        if tri.vertex_count() == 0 {
            return self;
        }

        // collect edge classifications for quick lookup
        let (main_edge_class_map, sub_edge_class_map) = {
            let mut main_edge_class_map = HashMap::new();
            let mut sub_edge_class_map = HashMap::new();

            for (edge_list, class, offset) in edges {
                for fe in edge_list {
                    if offset {
                        sub_edge_class_map.insert((fe.triangle, fe.edge), class);
                    } else {
                        main_edge_class_map.insert((fe.triangle, fe.edge), class);
                    }
                }
            }
            (main_edge_class_map, sub_edge_class_map)
        };

        for fi in tri.face_index_iter() {
            if fi.is_none() {
                continue;
            }
            let face = &tri[fi];

            // accumulate face center for labeling
            let mut face_center = Vec2::ZERO;
            let mut face_points = 0.;

            // Finite edges
            for i in 0..3 {
                let e = Rot3Idx::new(i);
                let v0_idx = face.vertices[e.increment()];
                let v1_idx = face.vertices[e.decrement()];

                if v0_idx.is_valid()
                    && v1_idx.is_valid()
                    && tri.is_finite_vertex(v0_idx)
                    && tri.is_finite_vertex(v1_idx)
                {
                    let p0 = tri[v0_idx].position.to_vec2();
                    let p1 = tri[v1_idx].position.to_vec2();
                    let pc = (p0 + p1) * 0.5;
                    let ed = (p1 - p0).normalize().perp();
                    face_center += p0;
                    face_center += p1;
                    face_points += 2.0;

                    let main_edge_class = main_edge_class_map.get(&(fi, e)).unwrap_or_else(|| {
                        if face.constraints[e] != 0 {
                            &"edge-constraint"
                        } else {
                            &"edge"
                        }
                    });
                    self.add_content(Content::Line {
                        p0,
                        p1,
                        class: main_edge_class.to_string(),
                    });

                    let nfi = face.neighbors[e];
                    let sub_edge_class = sub_edge_class_map.get(&(fi, e)).or_else(|| {
                        if nfi.is_valid() {
                            Some(&"edge-neighbor")
                        } else {
                            None
                        }
                    });
                    if let Some(sub_edge_class) = sub_edge_class {
                        self.add_content(Content::OffsetLine {
                            p0,
                            p1,
                            offset: ed,
                            class: sub_edge_class.to_string(),
                        });
                    }

                    if nfi.is_valid() {
                        self.add_content(Content::Text {
                            pos: pc,
                            offset: ed,
                            text: i.to_string(),
                            class: "edge-text".to_string(),
                        });
                    }
                }
            }

            // Infinite triangles (wedges)
            for i in 0..3 {
                let e = Rot3Idx::new(i);
                if tri.is_infinite_vertex(face.vertices[e]) {
                    let v_prev = face.vertices[e.increment()];
                    let v_next = face.vertices[e.decrement()];
                    if v_prev.is_valid()
                        && v_next.is_valid()
                        && tri.is_finite_vertex(v_prev)
                        && tri.is_finite_vertex(v_next)
                    {
                        let p_prev = tri[v_prev].position.to_vec2();
                        let p_next = tri[v_next].position.to_vec2();
                        let ld = (p_next - p_prev).perp() * 0.3;
                        let c = (p_prev + p_next) * 0.5 + ld;

                        // to avoid bias, add all the edge vertices using the virtual inf vertex position
                        face_center += p_prev;
                        face_center += c;
                        face_center += p_next;
                        face_center += c;
                        face_points += 4.0;

                        self.add_content(Content::Line {
                            p0: p_prev,
                            p1: c,
                            class: "edge-neighbor".to_string(),
                        });
                        self.add_content(Content::Line {
                            p0: c,
                            p1: p_next,
                            class: "edge-neighbor".to_string(),
                        });
                    }
                }
            }

            // Face labels
            if face_points > 0. {
                let face_center = face_center / face_points;
                let id_str = fi
                    .try_into_index()
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "None".to_string());
                self.add_content(Content::Text {
                    pos: face_center,
                    offset: Vec2::ZERO,
                    text: id_str,
                    class: "face-text".to_string(),
                });
            }
        } // for faces

        for vi in tri.vertex_index_iter() {
            if vi.is_none() || !tri.is_finite_vertex(vi) {
                continue;
            }
            let p = tri[vi].position.to_vec2();
            let id_str = vi
                .try_into_index()
                .map(|id| id.to_string())
                .unwrap_or_else(|| "None".to_string());
            // Use zero-length line with round linecap for zoom-independent point
            self.add_content(Content::Point { pos: p, class: "vert".into() });
            self.add_content(Content::Text {
                pos: p,
                offset: Vec2::ZERO,
                text: id_str,
                class: "vert-text".to_string(),
            });
        }

        self
    }

    /// Add a quadrangulation to the SVG dump, with optional edge classifications for highlighting.
    /// The `edges` parameter is an iterator of tuples containing:
    /// - A list of `QuadEdge` references that identify edges in the quadrangulation.
    /// - A CSS class name to apply for highlighting those edges.
    /// - A boolean indicating whether to use the edge or the neighbor edge for highlighting (false = edge, true = neighbor).
    pub fn add_quad<'a, ECI: IntoIterator<Item = (&'a [QuadEdge], &'a str, bool)>>(
        &mut self,
        quad: &Quadrangulation,
        edges: ECI,
    ) -> &mut Self {
        if quad.vertex_count() == 0 {
            return self;
        }

        // collect edge classifications for quick lookup
        let (main_edge_class_map, sub_edge_class_map) = {
            let mut main_edge_class_map = HashMap::new();
            let mut sub_edge_class_map = HashMap::new();

            for (edge_list, class, offset) in edges {
                for qe in edge_list {
                    if offset {
                        sub_edge_class_map.insert((qe.quad, qe.edge), class);
                    } else {
                        main_edge_class_map.insert((qe.quad, qe.edge), class);
                    }
                }
            }
            (main_edge_class_map, sub_edge_class_map)
        };

        // Edge offset is now handled in normalized space via ContentInfo
        // No need for local edge_offset calculation

        for qi in quad.quad_index_iter() {
            if qi.is_none() {
                continue;
            }
            let q = &quad[qi];

            // accumulate quad center for labeling
            let mut quad_center = Vec2::ZERO;
            let mut quad_points = 0.;

            // Finite edges
            for i in 0..4 {
                let e = Rot4Idx::new(i);
                let v0_idx = q.vertices[e];
                let v1_idx = q.vertices[e.increment()];

                if v0_idx.is_valid()
                    && v1_idx.is_valid()
                    && quad.is_finite_vertex(v0_idx)
                    && quad.is_finite_vertex(v1_idx)
                {
                    let p0 = quad.p(v0_idx);
                    let p1 = quad.p(v1_idx);
                    let pc = (p0 + p1) * 0.5;
                    let ed = (p1 - p0).normalize().perp();
                    quad_center += p0;
                    quad_center += p1;
                    quad_points += 2.0;

                    let main_edge_class = main_edge_class_map.get(&(qi, e)).unwrap_or(&"edge");
                    self.add_content(Content::Line {
                        p0,
                        p1,
                        class: main_edge_class.to_string(),
                    });

                    let nqi = q.neighbors[e];
                    let sub_edge_class = sub_edge_class_map.get(&(qi, e)).unwrap_or_else(|| {
                        if nqi.is_valid() {
                            &"edge-neighbor"
                        } else {
                            &"edge-neighbor-error"
                        }
                    });
                    self.add_content(Content::OffsetLine {
                        p0,
                        p1,
                        offset: ed,
                        class: sub_edge_class.to_string(),
                    });
                    self.add_content(Content::Text {
                        pos: pc,
                        offset: ed,
                        text: i.to_string(),
                        class: "edge-text".into(),
                    });
                }
            }

            // Infinite quads: visualize as wedges extending outward from boundary
            // Ghost quad format: [inf, v1, v0, v_prev]
            // The two finite edges are v1->v0 and v0->v_prev, meeting at v0
            let infinite_count = q.vertices.iter().filter(|&&v| quad.is_infinite_vertex(v)).count();
            if infinite_count > 1 {
                log::error!(
                    "WARNING: Quad {} has {} infinite vertices (expected 0 or 1), skipping ghost visualization",
                    qi.try_into_index().unwrap_or(999),
                    infinite_count
                );
            } else if infinite_count == 1 {
                // Find the infinite vertex position and collect finite vertices in quad order
                let inf_pos = quad[qi].find_vertex(quad.infinite_vertex()).unwrap();
                let vi0 = quad.vi(QuadVertex::new(qi, inf_pos.add(1)));
                let vi1 = quad.vi(QuadVertex::new(qi, inf_pos.add(2)));
                let vi2 = quad.vi(QuadVertex::new(qi, inf_pos.add(3)));
                let p0 = quad.p(vi0);
                let p1 = quad.p(vi1);
                let p2 = quad.p(vi2);
                let p_inf = self.add_quad_infinite_wedge(p0, p1, p2);
                quad_center = p_inf;
                quad_points = 1.0;
            }

            // Quad labels
            if quad_points > 0. {
                let quad_center = quad_center / quad_points;
                let id_str = qi
                    .try_into_index()
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "None".to_string());
                self.add_content(Content::Text {
                    pos: quad_center,
                    offset: Vec2::ZERO,
                    text: id_str,
                    class: "face-text".into(),
                });
            }
        } // for quads

        for vi in quad.vertex_index_iter() {
            if vi.is_none() || !quad.is_finite_vertex(vi) {
                continue;
            }
            let p = quad.p(vi);
            let id_str = vi
                .try_into_index()
                .map(|id| id.to_string())
                .unwrap_or_else(|| "None".to_string());
            // Use zero-length line with round linecap for zoom-independent point
            self.add_content(Content::Point { pos: p, class: "vert".into() });
            self.add_content(Content::Text {
                pos: p,
                offset: Vec2::ZERO,
                text: id_str,
                class: "vert-text".into(),
            });
        }

        self
    }

    /// Draw an infinite wedge for an infinite quad returning it's position
    fn add_quad_infinite_wedge(&mut self, v0: Vec2, v1: Vec2, v2: Vec2) -> Vec2 {
        let v10 = v0 - v1;
        let v12 = v2 - v1;
        let l = v10.length() + v12.length();
        let v10 = v10.normalize();
        let v12 = v12.normalize();
        let bisector = bisector(v10, v12);

        let ghost_pos = v1 - bisector * l * 0.3;

        self.add_content(Content::Line {
            p0: v0,
            p1: ghost_pos,
            class: "edge-neighbor".into(),
        });
        self.add_content(Content::Line {
            p0: v2,
            p1: ghost_pos,
            class: "edge-neighbor".into(),
        });

        ghost_pos
    }

    fn write_start(&self, stream: &mut dyn io::Write, bounds: (Vec2, Vec2)) -> io::Result<()> {
        let mut tl = bounds.0;
        let mut size = bounds.1 - bounds.0;

        // add some padding
        const PAD: f32 = 0.2;
        size *= 1.0 + PAD;
        tl -= size * (PAD * 0.5);

        writeln!(
            stream,
            r#"<svg viewBox="{:.2} {:.2} {:.2} {:.2}" xmlns="http://www.w3.org/2000/svg" style="background-color: #f8f8f8;">"#,
            tl.x, tl.y, size.x, size.y
        )?;
        Ok(())
    }

    fn write_end(&self, stream: &mut dyn io::Write) -> io::Result<()> {
        writeln!(stream, "</svg>")?;
        Ok(())
    }

    fn write_style(&self, stream: &mut dyn io::Write) -> io::Result<()> {
        writeln!(stream, "  <style>")?;
        writeln!(stream, "    <![CDATA[")?;
        for (class, style) in &self.styles {
            writeln!(stream, "    .{} {{ {} }}", class, style)?;
        }
        writeln!(stream, "    ]]>")?;
        writeln!(stream, "  </style>")?;

        Ok(())
    }
}
