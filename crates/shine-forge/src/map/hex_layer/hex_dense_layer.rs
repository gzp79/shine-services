use crate::map::{
    hex_layer::HexDenseIndexer, AxialCoord, HexLayer, HexLayerConfig, MapError, MapLayer, MapLayerIO, Tile,
    VoldemortIOToken,
};
use bevy::ecs::component::Component;
use serde::{Deserialize, Serialize};

/// A 2d hexagonal grid of tiles with dense memory layout.
#[derive(Component)]
pub struct HexDenseLayer<T>
where
    T: Tile,
{
    indexer: HexDenseIndexer,
    data: Vec<T>,
}

impl<T> HexDenseLayer<T>
where
    T: Tile,
{
    pub fn get_mut(&mut self, coord: AxialCoord) -> &mut T {
        if self.is_in_bounds(coord) {
            let index = self.indexer.get_dense_index(&coord);
            &mut self.data[index]
        } else {
            panic!("Out of bounds access")
        }
    }

    pub fn indexer(&self) -> &HexDenseIndexer {
        &self.indexer
    }

    pub fn data(&self) -> &[T] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [T] {
        &mut self.data
    }
}

impl<T> MapLayer for HexDenseLayer<T>
where
    T: Tile,
{
    type Tile = T;
    type Config = HexLayerConfig<T>;

    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            indexer: HexDenseIndexer::new(0),
            data: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.indexer = HexDenseIndexer::new(0);
        self.data.clear();
    }

    fn initialize(&mut self, config: &Self::Config) {
        let radius = config.radius;

        self.indexer = HexDenseIndexer::new(radius);
        let total_size = self.indexer.get_total_size();

        self.data.resize_with(total_size, <T as Default>::default);
    }

    fn is_empty(&self) -> bool {
        self.indexer.radius() == 0
    }
}

impl<T> HexLayer for HexDenseLayer<T>
where
    T: Tile,
{
    fn radius(&self) -> u32 {
        self.indexer.radius()
    }

    fn try_get(&self, coord: AxialCoord) -> Option<&Self::Tile> {
        if self.is_in_bounds(coord) {
            let index = self.indexer.get_dense_index(&coord);
            Some(&self.data[index])
        } else {
            None
        }
    }

    fn get(&self, coord: AxialCoord) -> &Self::Tile {
        self.try_get(coord).expect("Out of bounds access")
    }
}

impl<T> MapLayerIO for HexDenseLayer<T>
where
    T: Tile,
{
    fn load(&mut self, bytes: &[u8], _token: VoldemortIOToken) -> Result<(), MapError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[serde(bound = "U: Tile")]
        struct SnapshotV1<U>
        where
            U: Tile,
        {
            radius: u32,
            data: Vec<U>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[serde(bound = "U: Tile")]
        struct Snapshot<U>
        where
            U: Tile,
        {
            v1: Option<SnapshotV1<U>>,
        }

        let decoded: Snapshot<T> = rmp_serde::from_slice(bytes).map_err(MapError::LoadLayerError)?;
        if let Some(decoded) = decoded.v1 {
            self.indexer = HexDenseIndexer::new(decoded.radius);
            self.data = decoded.data;
            Ok(())
        } else {
            Err(MapError::LoadLayerError(rmp_serde::decode::Error::Syntax(
                "Unsupported snapshot version".into(),
            )))
        }
    }

    fn save(&self, _token: VoldemortIOToken) -> Result<Vec<u8>, MapError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        #[serde(bound = "U: Tile")]
        struct SnapshotLatest<'a, U>
        where
            U: Tile,
        {
            radius: u32,
            data: &'a [U],
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        #[serde(bound = "U: Tile")]
        struct Snapshot<'a, U>
        where
            U: Tile,
        {
            v1: Option<SnapshotLatest<'a, U>>,
        }

        rmp_serde::to_vec(&Snapshot {
            v1: Some(SnapshotLatest {
                radius: self.indexer.radius(),
                data: &self.data,
            }),
        })
        .map_err(MapError::SaveLayerError)
    }
}
