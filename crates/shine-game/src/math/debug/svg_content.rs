use glam::{vec2, Vec2};

pub struct ContentInfo {
    pub bounds: (Vec2, Vec2),
    pub normalized_bounds: (Vec2, Vec2),
    pub scale: f32,
}

impl ContentInfo {
    /// Transform a point from content space to normalized space
    /// Flips Y axis so Y increases upward (mathematical convention)
    fn normalize(&self, p: Vec2) -> Vec2 {
        let x = (p.x - self.bounds.0.x) * self.scale + self.normalized_bounds.0.x;
        // Flip Y: input Y increases upward, SVG Y increases downward
        let y = (self.bounds.1.y - p.y) * self.scale + self.normalized_bounds.0.y;
        Vec2::new(x, y)
    }
}

/// Escape special XML characters for safe inclusion in SVG
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub enum Content {
    Point {
        pos: Vec2,
        class: String,
    },
    Line {
        p0: Vec2,
        p1: Vec2,
        class: String,
    },
    OffsetLine {
        p0: Vec2,
        p1: Vec2,
        offset: Vec2,
        class: String,
    },
    Text {
        pos: Vec2,
        offset: Vec2,
        text: String,
        class: String,
    },
}

impl Content {
    pub fn get_bound(&self) -> (Vec2, Vec2) {
        match self {
            Content::Point { pos, .. } => (*pos, *pos),
            Content::Line { p0, p1, .. } => (p0.min(*p1), p0.max(*p1)),
            Content::OffsetLine { p0, p1, .. } => (p0.min(*p1), p0.max(*p1)),
            Content::Text { pos, .. } => (*pos, *pos),
        }
    }

    pub fn to_svg(&self, info: &ContentInfo) -> String {
        match self {
            Content::Point { pos, class } => {
                // Use zero-length line with round linecap for zoom-independent point
                let p = info.normalize(*pos);
                format!(
                    r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" class="{}" />"#,
                    p.x,
                    p.y,
                    p.x,
                    p.y,
                    xml_escape(class)
                )
            }
            Content::Line { p0, p1, class } => {
                let p0 = info.normalize(*p0);
                let p1 = info.normalize(*p1);
                format!(
                    r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" class="{}" />"#,
                    p0.x,
                    p0.y,
                    p1.x,
                    p1.y,
                    xml_escape(class)
                )
            }
            Content::OffsetLine { p0, p1, offset, class } => {
                let offset = vec2(offset.x, -offset.y);
                let p0 = info.normalize(*p0) + offset * 15.0;
                let p1 = info.normalize(*p1) + offset * 15.0;
                format!(
                    r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" class="{}"/>"#,
                    p0.x,
                    p0.y,
                    p1.x,
                    p1.y,
                    xml_escape(class)
                )
            }
            Content::Text { pos, offset, text, class } => {
                let offset = vec2(offset.x, -offset.y);
                let p = info.normalize(*pos) + offset * 12.5;
                format!(
                    r#"  <text x="{:.2}" y="{:.2}" class="{}">{}</text>"#,
                    p.x,
                    p.y,
                    xml_escape(class),
                    xml_escape(text)
                )
            }
        }
    }
}
