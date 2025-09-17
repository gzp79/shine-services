use crate::map::{MapError, MapLayer, RectCoord, RectDenseIndexer, RectLayer, RectLayerConfig, Tile};
use bevy::ecs::component::Component;

/// A 2d rectangular grid of tiles with dense memory layout.
#[derive(Component)]
pub struct RectDenseLayer<T>
where
    T: Tile,
{
    indexer: RectDenseIndexer,
    data: Vec<T>,
}

impl<T> RectDenseLayer<T>
where
    T: Tile,
{
    pub fn new(config: &RectLayerConfig<T>) -> Self {
        let (width, height) = (config.width, config.height);
        let indexer = RectDenseIndexer::new(width, height);
        let total_size = indexer.get_total_size();

        let mut data = Vec::with_capacity(total_size);
        data.resize_with(total_size, <T as Default>::default);

        Self { indexer, data }
    }

    pub fn get_mut(&mut self, coord: RectCoord) -> &mut T {
        if self.is_in_bounds(coord) {
            let index = self.indexer.get_dense_index(&coord);
            &mut self.data[index]
        } else {
            panic!("Out of bounds access")
        }
    }

    pub fn indexer(&self) -> &RectDenseIndexer {
        &self.indexer
    }

    pub fn data(&self) -> &[T] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [T] {
        &mut self.data
    }
}

impl<T> MapLayer for RectDenseLayer<T>
where
    T: Tile,
{
    type Tile = T;

    fn new_empty() -> Self
    where
        Self: Sized,
    {
        Self {
            indexer: RectDenseIndexer::new(0, 0),
            data: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.indexer.width() == 0 && self.indexer.height() == 0
    }

    fn clear(&mut self) {
        self.indexer = RectDenseIndexer::new(0, 0);
        self.data.clear();
    }

    fn load(&mut self, data: &[u8]) -> Result<(), MapError> {
        todo!()
    }

    fn save(&self) -> Vec<u8> {
        todo!()
    }
}

impl<T> From<RectLayerConfig<T>> for RectDenseLayer<T>
where
    T: Tile,
{
    fn from(config: RectLayerConfig<T>) -> Self {
        Self::new(&config)
    }
}

impl<T> RectLayer for RectDenseLayer<T>
where
    T: Tile,
{
    fn width(&self) -> u32 {
        self.indexer.width()
    }

    fn height(&self) -> u32 {
        self.indexer.height()
    }

    fn try_get(&self, coord: RectCoord) -> Option<&Self::Tile> {
        if self.is_in_bounds(coord) {
            let index = self.indexer.get_dense_index(&coord);
            Some(&self.data[index])
        } else {
            None
        }
    }

    fn get(&self, coord: RectCoord) -> &Self::Tile {
        self.try_get(coord).expect("Out of bounds access")
    }
}
