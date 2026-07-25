#![cfg(feature = "geoparquet")]
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;
use std::fs::File;

use arrow_array::{Array, BinaryArray, RecordBatch};
use geoparquet::metadata::GeoParquetMetadata;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::reader::{FileReader, SerializedFileReader};
use spatial_io::{
    AttributeValue, AxisDirection, CoordinateSpace, Crs, FeatureCollectionV1, FeatureV1,
    GeoParquetWriteOptions, GeometryV1, LineString, PixelAnchor, PixelOrigin, Point2,
    SpatialReference, write_geoparquet,
};

#[test]
fn writes_schema_valid_local_wkb_with_typed_attributes() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("local.parquet");
    let local = collection(
        CoordinateSpace::Local {
            unit: "canvas_unit".to_owned(),
        },
        vec![
            feature("a", line(&[(0.0, 0.0), (2.0, 3.0)])?, 7),
            feature("b", line(&[(-1.0, 2.0), (4.0, 5.0)])?, 9),
        ],
    );
    let report = write_geoparquet(&path, &local, GeoParquetWriteOptions::default())?;
    assert_eq!(report.feature_count, 2);
    assert_eq!(report.bbox, [-1.0, 0.0, 4.0, 5.0]);
    assert!(report.byte_length > 0);
    assert_eq!(report.sha256.len(), 64);

    let metadata = read_geo_metadata(&path)?;
    assert_eq!(metadata.version, "1.1.0");
    assert_eq!(metadata.primary_column, "geometry");
    let column = &metadata.columns["geometry"];
    let serialized = serde_json::to_value(column)?;
    assert_eq!(serialized["encoding"], "WKB");
    assert_eq!(
        serialized["geometry_types"],
        serde_json::json!(["LineString"])
    );
    assert_eq!(serialized["crs"], serde_json::Value::Null);
    assert_eq!(
        serialized["covering"]["bbox"]["xmin"],
        serde_json::json!(["bbox", "xmin"])
    );

    let batch = read_batch(&path)?;
    assert_eq!(
        batch.schema().field_with_name("class_id")?.data_type(),
        &arrow_schema::DataType::UInt64
    );
    let geometry = batch
        .column_by_name("geometry")
        .expect("geometry column")
        .as_any()
        .downcast_ref::<BinaryArray>()
        .expect("binary geometry");
    for index in 0..geometry.len() {
        let decoded = wkb::reader::read_wkb(geometry.value(index))?;
        assert_eq!(
            decoded.geometry_type(),
            wkb::reader::GeometryType::LineString
        );
    }
    Ok(())
}

#[test]
fn resolves_epsg_to_projjson_and_rejects_conflicts_before_publication()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("utm.parquet");
    let georeferenced = collection(
        CoordinateSpace::Georeferenced {
            crs: Crs::Epsg(32_618),
        },
        vec![feature(
            "a",
            line(&[(500_000.0, 4_400_000.0), (500_010.0, 4_400_020.0)])?,
            7,
        )],
    );
    write_geoparquet(&path, &georeferenced, GeoParquetWriteOptions::default())?;
    let metadata = read_geo_metadata(&path)?;
    let crs = serde_json::to_value(&metadata.columns["geometry"])?
        .get("crs")
        .cloned()
        .expect("explicit crs");
    assert_eq!(crs["id"]["authority"], "EPSG");
    assert_eq!(crs["id"]["code"], 32_618);

    let conflict_path = temp.path().join("conflict.parquet");
    let mut first = feature("a", line(&[(0.0, 0.0), (1.0, 1.0)])?, 7);
    first
        .attributes
        .insert("mixed".to_owned(), AttributeValue::U64(1));
    let mut second = feature("b", line(&[(1.0, 1.0), (2.0, 2.0)])?, 8);
    second
        .attributes
        .insert("mixed".to_owned(), AttributeValue::String("one".to_owned()));
    let conflicting = collection(pixel_space(), vec![first, second]);
    assert!(
        write_geoparquet(
            &conflict_path,
            &conflicting,
            GeoParquetWriteOptions::default()
        )
        .is_err()
    );
    assert!(!conflict_path.exists());
    Ok(())
}

#[test]
fn does_not_clobber_existing_destination_by_default() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("existing.parquet");
    std::fs::write(&path, b"unrelated")?;
    let collection = collection(
        pixel_space(),
        vec![feature("a", line(&[(0.0, 0.0), (1.0, 1.0)])?, 7)],
    );
    assert!(write_geoparquet(&path, &collection, GeoParquetWriteOptions::default()).is_err());
    assert_eq!(std::fs::read(&path)?, b"unrelated");
    Ok(())
}

fn feature(id: &str, line: LineString, class_id: u64) -> FeatureV1 {
    FeatureV1 {
        feature_id: id.to_owned(),
        source_primitive_id: format!("source-{id}"),
        geometry: GeometryV1::LineString(line),
        attributes: BTreeMap::from([("class_id".to_owned(), AttributeValue::U64(class_id))]),
        group_id: Some("fixture".to_owned()),
        conversion_profile_id: Some("recursive_convex_hull_bound_v1".to_owned()),
        conversion_tolerance: Some(0.25),
    }
}

fn collection(coordinate_space: CoordinateSpace, features: Vec<FeatureV1>) -> FeatureCollectionV1 {
    FeatureCollectionV1 {
        spatial_reference: SpatialReference {
            coordinate_space,
            affine: None,
            raster_interpretation: None,
        },
        features,
    }
}

fn pixel_space() -> CoordinateSpace {
    CoordinateSpace::Pixel {
        origin: PixelOrigin::TopLeft,
        y_axis: AxisDirection::Down,
        anchor: PixelAnchor::Corner,
    }
}

fn line(points: &[(f64, f64)]) -> Result<LineString, spatial_io::SpatialIoError> {
    LineString::new(
        points
            .iter()
            .map(|&(x, y)| Point2::new(x, y))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn read_geo_metadata(
    path: &std::path::Path,
) -> Result<GeoParquetMetadata, Box<dyn std::error::Error>> {
    let reader = SerializedFileReader::new(File::open(path)?)?;
    GeoParquetMetadata::from_parquet_meta(reader.metadata().file_metadata())
        .expect("geo metadata present")
        .map_err(Into::into)
}

fn read_batch(path: &std::path::Path) -> Result<RecordBatch, Box<dyn std::error::Error>> {
    let mut reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?.build()?;
    Ok(reader.next().expect("one batch")?)
}
