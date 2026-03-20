use crate::math::hex::{AxialCoord, AxialDenseIndexer, PatchDenseIndexer, PatchOrientation};
use glam::Vec2;
use std::fmt::Write;

/// Render the hex patch mesh to an SVG string for visualization.
/// Shows quad outlines with patch coloring, vertex dots, and patch boundary edges.
pub fn patch_mesh_to_svg(vertices: &[Vec2], subdivision: u32, orientation: PatchOrientation) -> String {
    let radius = 2u32.pow(subdivision);
    let indexer = AxialDenseIndexer::new(radius);
    let patch_indexer = PatchDenseIndexer::new(subdivision);

    // Compute viewBox from vertex bounds
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for pos in vertices.iter() {
        min_x = min_x.min(pos.x);
        min_y = min_y.min(pos.y);
        max_x = max_x.max(pos.x);
        max_y = max_y.max(pos.y);
    }
    let margin = (max_x - min_x).max(max_y - min_y) * 0.1;
    let vx = min_x - margin;
    let vy = min_y - margin;
    let vw = (max_x - min_x) + 2.0 * margin;
    let vh = (max_y - min_y) + 2.0 * margin;
    let stroke_w = vw * 0.003;
    let dot_r = vw * 0.005;

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vx:.4} {vy:.4} {vw:.4} {vh:.4}">"#,
    );

    // Draw quad outlines
    for i in 0..patch_indexer.get_total_size() {
        let patch = patch_indexer.get_coord(i);
        let quad = patch.quad_vertices(orientation, subdivision);
        let pts: Vec<Vec2> = quad.iter().map(|c| vertices[indexer.get_dense_index(c)]).collect();

        let points: String = pts
            .iter()
            .map(|p| format!("{:.4},{:.4}", p.x, p.y))
            .collect::<Vec<_>>()
            .join(" ");

        let _ = write!(
            svg,
            "  <polygon points=\"{points}\" fill=\"none\" \
             stroke=\"#666\" stroke-width=\"{stroke_w:.4}\"/>\n",
        );
    }

    // Draw boundary vertices
    for coord in AxialCoord::origin().ring(radius) {
        let pos = vertices[indexer.get_dense_index(&coord)];
        let _ = write!(
            svg,
            "  <circle cx=\"{:.4}\" cy=\"{:.4}\" r=\"{:.4}\" fill=\"#333\"/>\n",
            pos.x, pos.y, dot_r
        );
    }

    svg.push_str("</svg>\n");
    svg
}
