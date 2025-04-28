use crate::map2::{ChunkOperation, ChunkStore, Tile};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
#[serde(rename_all = "camelCase")]
pub struct SetTile<T>
where
    T: Tile + Serialize + DeserializeOwned,
{
    pub x: usize,
    pub y: usize,
    pub tile: T,
}

impl<T> ChunkOperation for SetTile<T>
where
    T: Tile + Serialize + DeserializeOwned,
{
    type Tile = T;

    fn apply<C>(self, chunk: &mut C)
    where
        C: ChunkStore<Tile = T>,
    {
        (*chunk.get_mut(self.x, self.y)) = self.tile;
    }
}
