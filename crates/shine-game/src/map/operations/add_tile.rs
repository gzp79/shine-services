use std::ops::AddAssign;

use crate::map::{ChunkOperation, ChunkStore, Tile};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
#[serde(rename_all = "camelCase")]
pub struct AddTile<T>
where
    T: Tile + Serialize + DeserializeOwned + AddAssign,
{
    pub x: usize,
    pub y: usize,
    pub tile: T,
}

impl<T> ChunkOperation for AddTile<T>
where
    T: Tile + Serialize + DeserializeOwned + AddAssign,
{
    type Tile = T;

    fn apply<C>(self, chunk: &mut C)
    where
        C: ChunkStore<Tile = T>,
    {
        log::trace!(
            "AddTile: x: {}, y: {}, tile: {:?}",
            self.x,
            self.y,
            serde_json::to_string(&self.tile).unwrap()
        );
        (*chunk.get_mut(self.x, self.y)) += self.tile;
    }
}
