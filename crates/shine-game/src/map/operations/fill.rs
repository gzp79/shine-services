use crate::map::{ChunkOperation, ChunkStore, Tile};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
#[serde(rename_all = "camelCase")]
pub struct Fill<T>
where
    T: Tile + Serialize + DeserializeOwned + Clone,
{
    pub tile: T,
}

impl<T> ChunkOperation for Fill<T>
where
    T: Tile + Serialize + DeserializeOwned + Clone,
{
    type Tile = T;

    fn apply<C>(self, chunk: &mut C)
    where
        C: ChunkStore<Tile = T>,
    {
        log::trace!("Fill, tile: {:?}", serde_json::to_string(&self.tile).unwrap());
        for (_, _, tile) in chunk.iter_mut() {
            *tile = self.tile.clone();
        }
    }
}
