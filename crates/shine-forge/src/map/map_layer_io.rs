use crate::map::{MapAuditedLayer, MapError};

/// Some token that prevents calling load/save directly outside of this module.
pub struct VoldemortIOToken(());

pub trait MapLayerIO: MapAuditedLayer {
    fn empty(
        &mut self,
        _token: VoldemortIOToken,
        config: &Self::Config,
        audit: Option<&mut Self::Audit>,
    ) -> Result<(), MapError>;

    fn load(
        &mut self,
        _token: VoldemortIOToken,
        config: &Self::Config,
        bytes: &[u8],
        audit: Option<&mut Self::Audit>,
    ) -> Result<(), MapError>;

    fn save(&self, _token: VoldemortIOToken, config: &Self::Config) -> Result<Vec<u8>, MapError>;
}

pub trait MapLayerIOExt: MapLayerIO {
    fn load_from_empty(&mut self, config: &Self::Config, audit: Option<&mut Self::Audit>) -> Result<(), MapError> {
        self.empty(VoldemortIOToken(()), config, audit)
    }

    fn load_from_bytes(
        &mut self,
        config: &Self::Config,
        bytes: &[u8],
        audit: Option<&mut Self::Audit>,
    ) -> Result<(), MapError> {
        let bytes = lz4_flex::decompress_size_prepended(bytes).map_err(MapError::DecompressLayerError)?;
        self.load(VoldemortIOToken(()), config, &bytes, audit)?;
        Ok(())
    }

    fn save_to_bytes(&self, config: &Self::Config) -> Result<Vec<u8>, MapError> {
        let bytes = self.save(VoldemortIOToken(()), config)?;
        let bytes = lz4_flex::compress_prepend_size(&bytes);
        Ok(bytes)
    }
}
impl<T> MapLayerIOExt for T where T: MapLayerIO {}
