use std::marker::PhantomData;

use crate::map2::{ChunkOperation, ChunkStore, Tile};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Nop<T>(pub PhantomData<T>)
where
    T: Tile + Clone;

impl<T> Nop<T>
where
    T: Tile + Clone,
{
    pub fn new() -> Self {
        Self::default()
    }
}

impl<T> ChunkOperation for Nop<T>
where
    T: Tile + Clone,
{
    type Tile = T;

    fn apply<C>(self, _chunk: &mut C)
    where
        C: ChunkStore<Tile = T>,
    {
    }
}
