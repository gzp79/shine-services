use crate::{
    indexed::TypedIndex,
    math::triangulation::{FaceEdge, FaceIndex, Rot3Idx, Triangulation},
};
use glam::{IVec2, Vec2};
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
    bounds: (Vec2, Vec2),
    content: Vec<String>,
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
            bounds: (Vec2::MAX, Vec2::MIN),
            content: Vec::new(),
            styles: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.bounds = (Vec2::MAX, Vec2::MIN);
        self.content.clear();
        self.styles.clear();
    }

    #[must_use]
    pub fn write(&self, stream: &mut dyn io::Write) -> io::Result<()> {
        let u1 = (self.bounds.1 - self.bounds.0).length() * 0.01;
        let u2 = u1 * 2.;
        let u4 = u1 * 4.;

        self.write_start(stream)?;
        self.write_style(stream)?;
        for chunk in &self.content {
            let chunk = chunk
                .replace("%u1%", &format!("{:.2}", u1))
                .replace("%u2%", &format!("{:.2}", u2))
                .replace("%u4%", &format!("{:.2}", u4));
            writeln!(stream, "{}", chunk)?;
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

    pub fn enlarge_bounds<V: Into<Vec2>>(&mut self, p: V) -> &mut Self {
        let p = p.into();
        self.bounds.0 = self.bounds.0.min(p);
        self.bounds.1 = self.bounds.1.max(p);
        self
    }

    pub fn add_style<C: ToString, S: ToString>(&mut self, class: C, style: S) -> &mut Self {
        self.styles.insert(class.to_string(), style.to_string());
        self
    }

    pub fn add_content<C: ToString>(&mut self, content: C) -> &mut Self {
        self.content.push(content.to_string());
        self
    }

    #[rustfmt::skip]
    pub fn add_default_styles(&mut self) -> &mut Self {
        self.add_style("vert", "fill: #2c3e50;");
        self.add_style("vert-inf", "fill: #e67e22;");
        self.add_style("vert-text", "font: bold 14px sans-serif; fill: #333; pointer-events: none;");
        self.add_style("edge", "stroke: #bbb; stroke-width: 1.0; stroke-linecap: round;");
        self.add_style("edge-neighbor", "stroke: #666; stroke-width: 0.5; stroke-dasharray: 2,2; opacity: 0.3;");
        self.add_style("edge-constraint", "stroke: #004c3c; stroke-width: 2.5; stroke-linecap: round;");
        self.add_style("edge-list", "stroke: #e7003c; stroke-width: 2.5; stroke-linecap: round;");
        self.add_style("edge-delaunay", "stroke: #e74c00; stroke-width: 2.5; stroke-linecap: round;");
        self.add_style("edge-delaunay-first", "stroke: #e70000; stroke-width: 2.5; stroke-linecap: round; opacity: 0.5;");
        self.add_style("edge-text", "font: 10px sans-serif; fill: #666; pointer-events: none; text-anchor: middle; dominant-baseline: middle;");
        self.add_style("face-text", "font: italic 12px sans-serif; fill: #999; pointer-events: none; text-anchor: middle; dominant-baseline: middle;");
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
            self.enlarge_bounds(p0);
            self.enlarge_bounds(p1);
            self.add_content(format!(
                r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" class="{stroke_class}" />"#,
                p0.x, p0.y, p1.x, p1.y
            ));
        }

        for (i, &p) in points.iter().enumerate() {
            self.enlarge_bounds(p);
            self.add_content(format!(
                r#"  <circle cx="{:.2}" cy="{:.2}" r="%u1%" class="{point_class}" />"#,
                p.x, p.y
            ));
            if text_class != "" {
                self.add_content(format!(
                    r#"  <text x="{:.2}" y="{:.2}" dx="%u1%" dy="-%u1%" class="{text_class}">{}</text>"#,
                    p.x, p.y, i
                ));
            }
        }

        self
    }

    pub fn add_contour<V: DebugVec2, IV: Iterator<Item = V>>(&mut self, points: IV, edge_class: &str) -> &mut Self {
        let edge_class = if edge_class == "" { "edge" } else { edge_class };

        let mut prev: Option<Vec2> = None;
        for p in points {
            let p: Vec2 = p.to_vec2();
            self.enlarge_bounds(p);
            if let Some(prev) = prev {
                self.add_content(format!(
                    r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" class="{edge_class}" />"#,
                    prev.x, prev.y, p.x, p.y
                ));
            }
            prev = Some(p);
        }
        self
    }

    pub fn add_tri<'a, I: IntoIterator<Item = (&'a Vec<FaceEdge>, &'a str)>, const D: bool>(
        &mut self,
        tri: &Triangulation<D>,
        edges: I,
    ) -> &mut Self {
        if tri.vertex_count() == 0 {
            return self;
        }

        // colect edge classifications for quick lookup
        let edge_class_map = {
            let mut edge_class_map = HashMap::new();
            for (edge_list, class) in edges {
                for &fe in edge_list {
                    edge_class_map.insert((fe.face, fe.edge), class);
                }
            }
            edge_class_map
        };

        for fi in tri.face_index_iter() {
            if fi.is_none() {
                continue;
            }
            let face = &tri[fi];

            // acumulate face center for labeling
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
                    face_center += p0;
                    face_center += p1;
                    face_points += 2.0;
                    self.enlarge_bounds(p0);
                    self.enlarge_bounds(p1);

                    let class = if face.constraints[e] != 0 {
                        "edge-constraint"
                    } else {
                        "edge"
                    };
                    self.add_content(format!(
                        r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" class="{}" />"#,
                        p0.x, p0.y, p1.x, p1.y, class
                    ));

                    let nfi = face.neighbors[e];
                    if nfi.is_valid() || edge_class_map.contains_key(&(fi, e)) {
                        let ed = (p1 - p0).normalize().perp();

                        let class = edge_class_map.get(&(fi, e)).unwrap_or(&"edge-neighbor");

                        self.add_content(format!(
                            r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" class="{}" />"#,
                            p0.x + ed.x * 1.,
                            p0.y + ed.y * 1.,
                            p1.x + ed.x * 1.,
                            p1.y + ed.y * 1.,
                            class
                        ));
                        self.add_content(format!(
                            r#"  <text x="{:.2}" y="{:.2}" class="edge-text" style="font-size: %u1%;">{}</text>"#,
                            pc.x + ed.x * 0.05,
                            pc.y + ed.y * 0.05,
                            i
                        ));
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

                        self.enlarge_bounds(c);
                        self.add_content(format!(
                            r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" class="edge-neighbor" />"#,
                            p_prev.x, p_prev.y, c.x, c.y
                        ));
                        self.add_content(format!(
                            r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" class="edge-neighbor" />"#,
                            p_next.x, p_next.y, c.x, c.y
                        ));
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
                self.add_content(format!(
                    r#"  <text x="{:.2}" y="{:.2}" class="face-text">{}</text>"#,
                    face_center.x, face_center.y, id_str
                ));
            }
        } // for faces

        for vi in tri.vertex_index_iter() {
            if vi.is_none() || !tri.is_finite_vertex(vi) {
                continue;
            }
            let p = tri[vi].position.to_vec2();
            self.enlarge_bounds(p);
            let id_str = vi
                .try_into_index()
                .map(|id| id.to_string())
                .unwrap_or_else(|| "None".to_string());
            self.add_content(format!(
                r#"  <circle cx="{:.2}" cy="{:.2}" r="%u1%" class="vert" />"#,
                p.x, p.y
            ));
            self.add_content(format!(
                r#"  <text x="{:.2}" y="{:.2}" class="vert-text">{}</text>"#,
                p.x, p.y, id_str
            ));
        }

        self
    }

    fn write_start(&self, stream: &mut dyn io::Write) -> io::Result<()> {
        let mut tl = self.bounds.0;
        let mut size = self.bounds.1 - self.bounds.0;

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
        for (class, style) in &self.styles {
            writeln!(stream, "    .{} {{ {} }}", class, style)?;
        }
        writeln!(stream, "  </style>")?;

        Ok(())
    }
}
