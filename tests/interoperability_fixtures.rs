#![cfg(feature = "geoparquet")]
#![allow(clippy::float_cmp)]

use std::fs::File;
use std::path::{Path, PathBuf};

use arrow_array::{Array, BinaryArray};
use geoparquet::metadata::GeoParquetMetadata;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::reader::{FileReader, SerializedFileReader};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Manifest {
    schema: String,
    license: String,
    generator: String,
    geometry_encoding: String,
    fixtures: Vec<FixtureRecord>,
}

#[derive(Deserialize)]
struct FixtureRecord {
    file: String,
    coordinate_space: String,
    expected_crs: Option<String>,
    geometry_types: Vec<String>,
    feature_count: usize,
    bbox: [f64; 4],
    byte_length: u64,
    sha256: String,
}

#[test]
fn checked_in_interoperability_matrix_is_attested_and_schema_valid()
-> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/interoperability");
    let manifest: Manifest = serde_json::from_reader(File::open(root.join("manifest.json"))?)?;

    assert_eq!(
        manifest.schema,
        "spatial_io_interoperability_fixture_matrix_v1"
    );
    assert_eq!(manifest.license, "MIT OR Apache-2.0");
    assert_eq!(
        manifest.generator,
        "examples/generate_interoperability_fixtures.rs"
    );
    assert_eq!(manifest.geometry_encoding, "GeoParquet 1.1 WKB");
    assert_eq!(manifest.fixtures.len(), 3);

    let mut expected_files = manifest
        .fixtures
        .iter()
        .map(|fixture| fixture.file.clone())
        .collect::<Vec<_>>();
    expected_files.sort();
    let mut actual_files = std::fs::read_dir(&root)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "parquet")
        })
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    actual_files.sort();
    assert_eq!(actual_files, expected_files);

    for fixture in &manifest.fixtures {
        verify_fixture(&root.join(&fixture.file), fixture)?;
    }
    Ok(())
}

fn verify_fixture(path: &Path, fixture: &FixtureRecord) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    assert_eq!(u64::try_from(bytes.len())?, fixture.byte_length);
    assert_eq!(hex::encode(Sha256::digest(&bytes)), fixture.sha256);
    assert_eq!(fixture.geometry_types, ["LineString"]);

    let file_reader = SerializedFileReader::new(File::open(path)?)?;
    let geo = GeoParquetMetadata::from_parquet_meta(file_reader.metadata().file_metadata())
        .expect("GeoParquet metadata present")?;
    assert_eq!(geo.version, "1.1.0");
    assert_eq!(geo.primary_column, "geometry");
    let column = serde_json::to_value(&geo.columns["geometry"])?;
    assert_eq!(column["encoding"], "WKB");
    assert_eq!(column["geometry_types"], serde_json::json!(["LineString"]));
    assert_eq!(column["bbox"], serde_json::json!(fixture.bbox));
    match fixture.expected_crs.as_deref() {
        None => assert_eq!(column["crs"], serde_json::Value::Null),
        Some("EPSG:32618") => {
            assert_eq!(column["crs"]["id"]["authority"], "EPSG");
            assert_eq!(column["crs"]["id"]["code"], 32_618);
        }
        Some(other) => panic!("unexpected fixture CRS {other}"),
    }

    let mut reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?.build()?;
    let batch = reader.next().expect("one record batch")?;
    assert_eq!(batch.num_rows(), fixture.feature_count);
    for required in [
        "feature_id",
        "source_primitive_id",
        "group_id",
        "conversion_profile_id",
        "conversion_tolerance",
        "class_id",
        "label",
        "score",
        "visible",
        "geometry",
        "bbox",
    ] {
        batch.schema().field_with_name(required)?;
    }
    let geometry = batch
        .column_by_name("geometry")
        .expect("geometry column")
        .as_any()
        .downcast_ref::<BinaryArray>()
        .expect("binary WKB geometry");
    for index in 0..geometry.len() {
        assert_eq!(
            wkb::reader::read_wkb(geometry.value(index))?.geometry_type(),
            wkb::reader::GeometryType::LineString
        );
    }

    assert!(matches!(
        fixture.coordinate_space.as_str(),
        "pixel_top_left_y_down_corner" | "local_millimetre" | "georeferenced"
    ));
    Ok(())
}
