use crate::map::MapError;

/// Some token that prevents calling load/save directly outside of this module.
pub struct VoldemortIOToken(());

pub trait MapLayerIO {
    fn load(&mut self, data: &[u8], _token: VoldemortIOToken) -> Result<(), MapError>;

    fn save(&self, _token: VoldemortIOToken) -> Result<Vec<u8>, MapError>;
}

pub trait MapLayerIOExt: MapLayerIO {
    fn load_from_bytes(&mut self, bytes: &[u8]) -> Result<(), MapError> {
        let bytes = lz4_flex::decompress_size_prepended(bytes).map_err(MapError::DecompressLayerError)?;
        self.load(&bytes, VoldemortIOToken(()))?;
        Ok(())
    }

    fn save_to_bytes(&self) -> Result<Vec<u8>, MapError> {
        let bytes = self.save(VoldemortIOToken(()))?;
        let bytes = lz4_flex::compress_prepend_size(&bytes);
        Ok(bytes)
    }
}
impl<T> MapLayerIOExt for T where T: MapLayerIO {}
