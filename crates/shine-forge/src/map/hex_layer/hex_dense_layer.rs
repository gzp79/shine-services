use crate::map::{hex_layer::HexDenseIndexer, AxialCoord, HexLayer, HexLayerConfig, MapError, MapLayer, Tile};
use bevy::ecs::component::Component;

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
    pub fn new(config: &HexLayerConfig<T>) -> Self {
        let radius = config.radius;
        let indexer = HexDenseIndexer::new(radius);
        let total_size = indexer.get_total_size();

        let mut data = Vec::with_capacity(total_size);
        data.resize_with(total_size, <T as Default>::default);

        Self { indexer, data }
    }

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

    fn new_empty() -> Self
    where
        Self: Sized,
    {
        Self {
            indexer: HexDenseIndexer::new(0),
            data: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.indexer.radius() == 0
    }

    fn clear(&mut self) {
        self.indexer = HexDenseIndexer::new(0);
        self.data.clear();
    }

    fn load(&mut self, data: &[u8]) -> Result<(), MapError> {
        todo!()
    }

    fn save(&self) -> Vec<u8> {
        todo!()
    }
}

impl<T> From<HexLayerConfig<T>> for HexDenseLayer<T>
where
    T: Tile,
{
    fn from(config: HexLayerConfig<T>) -> Self {
        Self::new(&config)
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
