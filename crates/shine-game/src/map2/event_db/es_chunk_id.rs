use crate::map2::ChunkId;
use shine_infra::db::event_source::StreamId;

impl StreamId for ChunkId {
    fn from_string(id: String) -> Self {
        let parts: Vec<&str> = id.split('-').collect();
        if parts.len() != 2 {
            panic!("Invalid ChunkId format");
        }
        let x = parts[0]
            .parse::<usize>()
            .expect("Invalid ChunkId format - x coordinate");
        let y = parts[1]
            .parse::<usize>()
            .expect("Invalid ChunkId format - y coordinate");
        ChunkId(x, y)
    }

    fn to_string(&self) -> String {
        format!("{}-{}", self.0, self.1)
    }
}
