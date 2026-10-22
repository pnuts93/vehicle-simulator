//! Decodes a single-band elevation GeoTIFF into a [`Heightmap`]. Knows nothing about Bevy's ECS
//! or assets.
//!
//! Supports single-band rasters of 32/64-bit float or 16-bit integer samples that carry a
//! `ModelPixelScale` tag. Elevations are taken as metres; cell sizes come from the pixel scale and
//! are assumed to be metres too, which holds for projected CRSs such as UTM but not for geographic
//! (degree) ones.

use std::{fmt, io::Cursor};

use bevy::math::{DVec2, Vec2};
use tiff::{
    decoder::{Decoder, DecodingResult},
    tags::Tag,
};

use super::heightmap::{Heightmap, HeightmapError};

/// Where the raster sits on the Earth.
#[derive(Clone, Debug, PartialEq)]
pub struct GeoReference {
    /// Projected coordinates (easting, northing) of the north-west corner of the first sample.
    pub origin: DVec2,
    /// EPSG code of the projected CRS, when the file declares one (e.g. 32632 for UTM 32N).
    pub epsg: Option<u16>,
}

#[derive(Debug)]
pub struct DecodedGeoTiff {
    pub heightmap: Heightmap,
    pub geo: GeoReference,
}

#[derive(Debug)]
pub enum GeoTiffError {
    Tiff(tiff::TiffError),
    /// The file has no pixel scale, so cell sizes are unknown.
    MissingGeoreferencing,
    /// Only single-band rasters are elevation models.
    NotSingleBand,
    UnsupportedSampleType,
    /// Every sample equals the nodata value.
    NoValidSamples,
    Heightmap(HeightmapError),
}

impl fmt::Display for GeoTiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tiff(e) => write!(f, "invalid TIFF: {e}"),
            Self::MissingGeoreferencing => write!(f, "TIFF has no GeoTIFF pixel scale"),
            Self::NotSingleBand => write!(f, "elevation TIFF must have exactly one band"),
            Self::UnsupportedSampleType => write!(f, "unsupported elevation sample type"),
            Self::NoValidSamples => write!(f, "TIFF contains only nodata samples"),
            Self::Heightmap(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for GeoTiffError {}

impl From<tiff::TiffError> for GeoTiffError {
    fn from(e: tiff::TiffError) -> Self {
        Self::Tiff(e)
    }
}

impl From<HeightmapError> for GeoTiffError {
    fn from(e: HeightmapError) -> Self {
        Self::Heightmap(e)
    }
}

const GEO_KEY_PROJECTED_CS_TYPE: u16 = 3072;

pub fn decode(bytes: &[u8]) -> Result<DecodedGeoTiff, GeoTiffError> {
    let mut decoder = Decoder::new(Cursor::new(bytes))?;
    let (cols, rows) = decoder.dimensions()?;
    if !matches!(decoder.colortype()?, tiff::ColorType::Gray(_)) {
        return Err(GeoTiffError::NotSingleBand);
    }

    let scale = decoder
        .get_tag_f64_vec(Tag::ModelPixelScaleTag)
        .ok()
        .filter(|s| s.len() >= 2)
        .ok_or(GeoTiffError::MissingGeoreferencing)?;
    // Tiepoint (i, j, k, x, y, z): raster position (i, j) is at model position (x, y).
    let origin = match decoder.get_tag_f64_vec(Tag::ModelTiepointTag) {
        Ok(t) if t.len() >= 6 => DVec2::new(t[3] - t[0] * scale[0], t[4] + t[1] * scale[1]),
        _ => DVec2::ZERO,
    };
    let epsg = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .ok()
        .and_then(|keys| projected_epsg(&keys));
    let nodata = decoder
        .get_tag_ascii_string(Tag::GdalNodata)
        .ok()
        .and_then(|s| s.trim_end_matches('\0').trim().parse::<f32>().ok());

    let mut elevations: Vec<f32> = match decoder.read_image()? {
        DecodingResult::F32(v) => v,
        DecodingResult::F64(v) => v.into_iter().map(|e| e as f32).collect(),
        DecodingResult::I16(v) => v.into_iter().map(f32::from).collect(),
        DecodingResult::U16(v) => v.into_iter().map(f32::from).collect(),
        _ => return Err(GeoTiffError::UnsupportedSampleType),
    };
    fill_voids(&mut elevations, nodata)?;

    let heightmap = Heightmap::new(
        cols as usize,
        rows as usize,
        Vec2::new(scale[0] as f32, scale[1] as f32),
        elevations,
    )?;
    Ok(DecodedGeoTiff {
        heightmap,
        geo: GeoReference { origin, epsg },
    })
}

/// Reads the `ProjectedCSTypeGeoKey` out of a GeoKeyDirectory (header of 4 shorts, then entries
/// of 4 shorts: key, tag location, count, value).
fn projected_epsg(keys: &[u16]) -> Option<u16> {
    keys.get(4..)?
        .chunks_exact(4)
        .find(|k| k[0] == GEO_KEY_PROJECTED_CS_TYPE && k[1] == 0)
        .map(|k| k[3])
}

/// Replaces nodata (and NaN) samples with the lowest valid elevation, so the terrain stays closed
/// instead of showing holes or spikes.
fn fill_voids(elevations: &mut [f32], nodata: Option<f32>) -> Result<(), GeoTiffError> {
    let is_void = |e: f32| e.is_nan() || nodata.is_some_and(|n| e == n);
    let floor = elevations
        .iter()
        .copied()
        .filter(|&e| !is_void(e))
        .fold(f32::MAX, f32::min);
    if floor == f32::MAX {
        return Err(GeoTiffError::NoValidSamples);
    }
    for e in elevations.iter_mut().filter(|e| is_void(**e)) {
        *e = floor;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voids_become_the_lowest_valid_elevation() {
        let mut e = [5., -9999., 3., f32::NAN, 8.];
        fill_voids(&mut e, Some(-9999.)).unwrap();
        assert_eq!(e, [5., 3., 3., 3., 8.]);
    }

    #[test]
    fn all_void_is_an_error() {
        let mut e = [-9999.; 4];
        assert!(matches!(
            fill_voids(&mut e, Some(-9999.)),
            Err(GeoTiffError::NoValidSamples)
        ));
    }

    #[test]
    fn finds_the_projected_crs_key() {
        let dir = [1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32632];
        assert_eq!(projected_epsg(&dir), Some(32632));
        assert_eq!(projected_epsg(&dir[..8]), None);
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(decode(b"not a tiff"), Err(GeoTiffError::Tiff(_))));
    }
}
