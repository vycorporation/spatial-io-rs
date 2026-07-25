//! Optional `GeoTIFF` spatial-reference input.

use std::path::{Path, PathBuf};

use geotiff_reader::crs::{CrsKind, RasterType};

use crate::{Affine2D, Crs, RasterInterpretation, SpatialIoError};

/// Crate-owned spatial metadata read from a local `GeoTIFF`.
#[derive(Debug, Clone, PartialEq)]
pub struct GeoTiffReference {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Number of storage-domain bands.
    pub band_count: u32,
    /// GDAL nodata text, when present.
    pub nodata: Option<String>,
    /// Exact, corner-normalized six-coefficient affine.
    pub affine: Affine2D,
    /// Original `GeoTIFF` raster-space interpretation.
    pub raster_interpretation: RasterInterpretation,
    /// Supported horizontal CRS identity.
    pub crs: Crs,
    /// Stable adapter provenance.
    pub adapter_id: &'static str,
    /// Source path supplied by the caller.
    pub source_path: PathBuf,
}

/// Reads only spatial-reference and basic raster metadata from a local `GeoTIFF`.
///
/// # Errors
///
/// Returns a typed error for unreadable input, absent or invalid affine
/// metadata, unsupported raster interpretation, or an unsupported CRS.
pub fn read_geotiff_reference(path: impl AsRef<Path>) -> Result<GeoTiffReference, SpatialIoError> {
    let path = path.as_ref();
    let file =
        geotiff_reader::GeoTiffFile::open(path).map_err(|error| SpatialIoError::GeoTiff {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    let transform = file.transform().ok_or(SpatialIoError::MissingAffine)?;
    let affine = Affine2D::new(
        transform.origin_x,
        transform.pixel_width,
        transform.skew_x,
        transform.origin_y,
        transform.skew_y,
        transform.pixel_height,
    )?;
    let raster_interpretation = match file.crs().raster_type_enum() {
        RasterType::PixelIsArea => RasterInterpretation::PixelIsArea,
        RasterType::PixelIsPoint => RasterInterpretation::PixelIsPoint,
        RasterType::Unknown(code) => {
            return Err(SpatialIoError::GeoTiff {
                path: path.to_owned(),
                message: format!("unsupported raster type code {code}"),
            });
        }
    };
    match file.crs().crs_kind() {
        CrsKind::Horizontal { .. } => {}
        CrsKind::Compound { .. } => {
            return Err(SpatialIoError::UnsupportedCrs(
                "compound horizontal/vertical GeoTIFF CRS is not yet supported".to_owned(),
            ));
        }
        CrsKind::Vertical(_) => {
            return Err(SpatialIoError::UnsupportedCrs(
                "vertical-only GeoTIFF CRS cannot georeference 2D linework".to_owned(),
            ));
        }
        CrsKind::Unspecified => return Err(SpatialIoError::MissingCrs),
    }
    let crs = Crs::epsg(file.epsg().ok_or(SpatialIoError::MissingCrs)?)?;
    Ok(GeoTiffReference {
        width: file.width(),
        height: file.height(),
        band_count: file.band_count(),
        nodata: file.nodata().map(ToOwned::to_owned),
        affine,
        raster_interpretation,
        crs,
        adapter_id: "geotiff_reader_0_7_reference_v1",
        source_path: path.to_owned(),
    })
}
