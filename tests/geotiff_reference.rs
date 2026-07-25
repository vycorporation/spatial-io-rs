#![cfg(feature = "geotiff")]
#![allow(clippy::float_cmp)]

use geotiff_writer::{GeoTiffBuilder, GeoTransform, RasterType, TiffVariant};
use ndarray::Array2;
use spatial_io::{Crs, RasterInterpretation, read_geotiff_reference};

#[test]
fn reads_north_up_pixel_is_area_reference() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("area.tif");
    let data = Array2::<u8>::zeros((3, 4));
    GeoTiffBuilder::new(4, 3)
        .projected_epsg(32_618)
        .raster_type(RasterType::PixelIsArea)
        .pixel_scale(2.0, 3.0)
        .origin(100.0, 200.0)
        .nodata("255")
        .write_2d(&path, data.view())?;

    let reference = read_geotiff_reference(&path)?;
    assert_eq!((reference.width, reference.height), (4, 3));
    assert_eq!(reference.band_count, 1);
    assert_eq!(reference.nodata.as_deref(), Some("255"));
    assert_eq!(
        reference.raster_interpretation,
        RasterInterpretation::PixelIsArea
    );
    assert_eq!(reference.crs, Crs::Epsg(32_618));
    assert_eq!(
        [
            reference.affine.origin_x,
            reference.affine.x_scale,
            reference.affine.x_skew,
            reference.affine.origin_y,
            reference.affine.y_skew,
            reference.affine.y_scale,
        ],
        [100.0, 2.0, 0.0, 200.0, 0.0, -3.0]
    );
    Ok(())
}

#[test]
fn preserves_pixel_is_point_and_rotation() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let point_path = temp.path().join("point.tif");
    let data = Array2::<u8>::zeros((2, 2));
    GeoTiffBuilder::new(2, 2)
        .projected_epsg(32_618)
        .raster_type(RasterType::PixelIsPoint)
        .pixel_scale(2.0, 4.0)
        .origin(100.0, 200.0)
        .write_2d(&point_path, data.view())?;
    let point_reference = read_geotiff_reference(&point_path)?;
    assert_eq!(
        point_reference.raster_interpretation,
        RasterInterpretation::PixelIsPoint
    );
    assert_eq!(point_reference.affine.origin_x, 100.0);
    assert_eq!(point_reference.affine.origin_y, 200.0);

    let rotated_path = temp.path().join("rotated.tif");
    let transform = GeoTransform {
        origin_x: 10.0,
        pixel_width: 2.0,
        skew_x: 0.25,
        origin_y: 20.0,
        skew_y: -0.5,
        pixel_height: -3.0,
    };
    GeoTiffBuilder::new(2, 2)
        .projected_epsg(32_618)
        .transform(transform)
        .write_2d(&rotated_path, data.view())?;
    let rotated = read_geotiff_reference(&rotated_path)?;
    assert_eq!(rotated.affine.x_skew, 0.25);
    assert_eq!(rotated.affine.y_skew, -0.5);
    Ok(())
}

#[test]
fn reads_bigtiff_and_tiled_layouts() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let data = Array2::<u8>::zeros((32, 32));
    let bigtiff = temp.path().join("big.tif");
    GeoTiffBuilder::new(32, 32)
        .epsg(4326)
        .pixel_scale(0.1, 0.1)
        .origin(-180.0, 90.0)
        .tiff_variant(TiffVariant::BigTiff)
        .write_2d(&bigtiff, data.view())?;
    assert_eq!(read_geotiff_reference(&bigtiff)?.crs, Crs::Epsg(4326));

    let tiled = temp.path().join("tiled.tif");
    GeoTiffBuilder::new(32, 32)
        .epsg(4326)
        .pixel_scale(0.1, 0.1)
        .origin(-180.0, 90.0)
        .tile_size(16, 16)
        .write_2d(&tiled, data.view())?;
    assert_eq!(read_geotiff_reference(&tiled)?.width, 32);
    Ok(())
}
