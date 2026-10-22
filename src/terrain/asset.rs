//! Bevy asset integration: `.tif` files load as [`TerrainTile`]. The decoding itself lives in
//! [`geotiff`](super::geotiff); this is only the glue to the asset server.

use bevy::{
    asset::{Asset, AssetLoader, LoadContext, io::Reader},
    reflect::TypePath,
};

use super::{
    geotiff::{self, GeoReference, GeoTiffError},
    heightmap::Heightmap,
};

/// One rectangular piece of terrain.
#[derive(Asset, TypePath, Debug)]
pub struct TerrainTile {
    pub heightmap: Heightmap,
    /// Where the tile sits on the Earth, for providers that stitch tiles together.
    pub geo: GeoReference,
}

#[derive(Default, TypePath)]
pub struct TerrainTileLoader;

#[derive(Debug)]
pub enum TerrainTileLoadError {
    Io(std::io::Error),
    GeoTiff(GeoTiffError),
}

impl std::fmt::Display for TerrainTileLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not read terrain file: {e}"),
            Self::GeoTiff(e) => write!(f, "could not decode terrain: {e}"),
        }
    }
}

impl std::error::Error for TerrainTileLoadError {}

impl AssetLoader for TerrainTileLoader {
    type Asset = TerrainTile;
    type Settings = ();
    type Error = TerrainTileLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<TerrainTile, TerrainTileLoadError> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(TerrainTileLoadError::Io)?;
        let decoded = geotiff::decode(&bytes).map_err(TerrainTileLoadError::GeoTiff)?;
        Ok(TerrainTile {
            heightmap: decoded.heightmap,
            geo: decoded.geo,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["tif", "tiff"]
    }
}
