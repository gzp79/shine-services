pub trait AsPolygonMesh {
    fn vertices(&self) -> &[f32];
    fn indices(&self) -> &[u32];
    fn ranges(&self) -> &[u32];
}

pub trait AsWiredPolygonMesh: AsPolygonMesh {
    fn wire_indices(&self) -> &[u32];
    fn wire_ranges(&self) -> &[u32];
}
