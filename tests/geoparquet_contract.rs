#![cfg(feature = "geoparquet")]
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;
use std::fs::File;

use arrow_array::{Array, BinaryArray, RecordBatch};
use geoparquet::metadata::GeoParquetMetadata;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::reader::{FileReader, SerializedFileReader};
use sha2::{Digest, Sha256};
use spatial_io::{
    AttributeFieldV1, AttributeType, AttributeValue, AxisDirection, CoordinateSpace, Crs,
    FeatureCollectionV1, FeatureV1, GeoParquetWriteOptions, GeometryV1, LineString, PixelAnchor,
    PixelOrigin, Point2, SpatialReference, write_geoparquet,
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
    assert_eq!(crs["base_crs"]["id"]["authority"], "EPSG");
    assert_eq!(crs["base_crs"]["id"]["code"], 4326);
    assert_eq!(
        crs["base_crs"]["coordinate_system"]["subtype"],
        "ellipsoidal"
    );
    assert_eq!(
        crs["base_crs"]["coordinate_system"]["axis"]
            .as_array()
            .expect("base CRS axes")
            .len(),
        2
    );
    let repeated_path = temp.path().join("utm-repeated.parquet");
    write_geoparquet(
        &repeated_path,
        &georeferenced,
        GeoParquetWriteOptions::default(),
    )?;
    assert_eq!(std::fs::read(&path)?, std::fs::read(&repeated_path)?);

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

#[test]
fn rejects_invalid_projjson_before_replacing_a_destination()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("invalid-crs.parquet");
    std::fs::write(&path, b"unrelated")?;
    for json in [
        "{}",
        r#"{"type":"GeographicCRS","name":"incomplete"}"#,
        r#"{"type":"GeographicCRS","name":"broken","datum":{},"coordinate_system":{}}"#,
    ] {
        let collection = collection(
            CoordinateSpace::Georeferenced {
                crs: Crs::ProjJson(json.to_owned()),
            },
            vec![feature("a", line(&[(0.0, 0.0), (1.0, 1.0)])?, 7)],
        );
        assert!(matches!(
            write_geoparquet(
                &path,
                &collection,
                GeoParquetWriteOptions { overwrite: true }
            ),
            Err(spatial_io::SpatialIoError::InvalidProjJson(_))
        ));
        assert_eq!(std::fs::read(&path)?, b"unrelated");
    }
    Ok(())
}

#[test]
fn accepts_valid_caller_projjson_and_rejects_nested_schema_violations()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let mut crs: serde_json::Value = serde_json::from_str(epsg_utils::epsg_to_projjson(4326)?)?;
    let valid_path = temp.path().join("caller-crs.parquet");
    let make_collection = |crs: &serde_json::Value| {
        collection(
            CoordinateSpace::Georeferenced {
                crs: Crs::projjson(crs.to_string()).unwrap(),
            },
            vec![feature(
                "a",
                line(&[(-75.0, 40.0), (-74.0, 41.0)]).unwrap(),
                7,
            )],
        )
    };
    write_geoparquet(
        &valid_path,
        &make_collection(&crs),
        GeoParquetWriteOptions::default(),
    )?;
    let encoded = serde_json::to_value(read_geo_metadata(&valid_path)?)?;
    assert_eq!(encoded["columns"]["geometry"]["crs"], crs);
    crs["coordinate_system"]["axis"][0]["direction"] = serde_json::json!("invalid-direction");
    let path = temp.path().join("invalid-axis.parquet");
    assert!(matches!(
        write_geoparquet(
            &path,
            &make_collection(&crs),
            GeoParquetWriteOptions::default()
        ),
        Err(spatial_io::SpatialIoError::InvalidProjJson(_))
    ));
    assert!(!path.exists());
    Ok(())
}

#[test]
fn rejects_mutated_affine_provenance_before_publication() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("invalid-affine.parquet");
    let mut input = collection(
        pixel_space(),
        vec![feature("a", line(&[(0.0, 0.0), (1.0, 1.0)])?, 7)],
    );
    let mut affine = spatial_io::Affine2D::new(0.0, 1.0, 0.0, 0.0, 0.0, 1.0)?;
    affine.y_scale = 0.0;
    input.spatial_reference.affine = Some(affine);
    assert!(matches!(
        write_geoparquet(&path, &input, GeoParquetWriteOptions::default()),
        Err(spatial_io::SpatialIoError::InvalidAffine(_))
    ));
    assert!(!path.exists());
    Ok(())
}

#[test]
fn preserves_complete_spatial_reference_in_file_metadata() -> Result<(), Box<dyn std::error::Error>>
{
    use spatial_io::{Affine2D, RasterInterpretation};
    let temp = tempfile::tempdir()?;
    let mut local = collection(
        CoordinateSpace::Local {
            unit: "millimetre".to_owned(),
        },
        vec![feature("a", line(&[(0.0, 0.0), (1.0, 1.0)])?, 7)],
    );
    let local_path = temp.path().join("local.parquet");
    write_geoparquet(&local_path, &local, GeoParquetWriteOptions::default())?;
    local.spatial_reference = SpatialReference {
        coordinate_space: CoordinateSpace::Pixel {
            origin: PixelOrigin::BottomLeft,
            y_axis: AxisDirection::Up,
            anchor: PixelAnchor::Center,
        },
        affine: Some(Affine2D::new(100.0, 2.0, 0.25, 200.0, -0.5, -3.0)?),
        raster_interpretation: Some(RasterInterpretation::PixelIsPoint),
    };
    let path = temp.path().join("pixel.parquet");
    write_geoparquet(&path, &local, GeoParquetWriteOptions::default())?;
    assert_ne!(
        Sha256::digest(std::fs::read(local_path)?),
        Sha256::digest(std::fs::read(&path)?)
    );
    let reader = SerializedFileReader::new(File::open(path)?)?;
    let metadata = reader
        .metadata()
        .file_metadata()
        .key_value_metadata()
        .unwrap();
    let reference: serde_json::Value = serde_json::from_str(
        metadata
            .iter()
            .find(|kv| kv.key == "spatial_io")
            .expect("spatial provenance")
            .value
            .as_deref()
            .unwrap(),
    )?;
    assert_eq!(reference["schema"], "spatial_io_spatial_reference_v1");
    let decoded: SpatialReference = serde_json::from_value(reference["spatial_reference"].clone())?;
    assert_eq!(decoded, local.spatial_reference);
    assert_eq!(
        serde_json::to_value(read_geo_metadata(&temp.path().join("pixel.parquet"))?)?["columns"]["geometry"]
            ["crs"],
        serde_json::Value::Null
    );
    Ok(())
}

#[test]
fn concurrent_overwrite_reports_attest_their_own_input() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::{Arc, Barrier};
    let temp = tempfile::tempdir()?;
    let target = Arc::new(temp.path().join("concurrent.parquet"));
    let inputs = (0..4)
        .map(|i| {
            collection(
                pixel_space(),
                vec![feature(
                    &format!("feature-{i}"),
                    line(&[(f64::from(i), 0.0), (f64::from(i), 1.0)]).unwrap(),
                    7,
                )],
            )
        })
        .collect::<Vec<_>>();
    let expected = inputs
        .iter()
        .enumerate()
        .map(|(i, input)| {
            write_geoparquet(
                temp.path().join(format!("expected-{i}.parquet")),
                input,
                GeoParquetWriteOptions::default(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let barrier = Arc::new(Barrier::new(inputs.len()));
    let handles = inputs
        .into_iter()
        .zip(expected)
        .map(|(input, expected)| {
            let target = target.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut correct = true;
                for _ in 0..32 {
                    barrier.wait();
                    // Every thread must complete its barriers, including failures.
                    correct &= write_geoparquet(
                        target.as_path(),
                        &input,
                        GeoParquetWriteOptions { overwrite: true },
                    )
                    .is_ok_and(|report| {
                        report.sha256 == expected.sha256
                            && report.byte_length == expected.byte_length
                    });
                }
                correct
            })
        })
        .collect::<Vec<_>>();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert!(results.into_iter().all(|correct| correct));
    Ok(())
}

#[test]
fn writes_declared_all_null_float_column_without_guessing() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("all-null.parquet");
    let mut collection = collection(
        pixel_space(),
        vec![feature("a", line(&[(0.0, 0.0), (1.0, 1.0)])?, 7)],
    );
    collection.attribute_schema.push(AttributeFieldV1 {
        name: "maximum_deviation".to_owned(),
        value_type: AttributeType::F64,
        nullable: true,
    });
    collection.features[0]
        .attributes
        .insert("maximum_deviation".to_owned(), AttributeValue::Null);

    write_geoparquet(&path, &collection, GeoParquetWriteOptions::default())?;
    let batch = read_batch(&path)?;
    let schema = batch.schema();
    let field = schema.field_with_name("maximum_deviation")?;
    assert_eq!(field.data_type(), &arrow_schema::DataType::Float64);
    assert!(field.is_nullable());
    assert_eq!(
        batch
            .column_by_name("maximum_deviation")
            .expect("declared column")
            .null_count(),
        1
    );
    Ok(())
}

#[test]
fn rejects_duplicate_reserved_undeclared_and_mismatched_attribute_fields()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let base = collection(
        pixel_space(),
        vec![feature("a", line(&[(0.0, 0.0), (1.0, 1.0)])?, 7)],
    );

    let mut duplicate = base.clone();
    duplicate
        .attribute_schema
        .push(duplicate.attribute_schema[0].clone());
    assert_rejected(&temp.path().join("duplicate.parquet"), &duplicate);

    let mut reserved = base.clone();
    reserved.attribute_schema[0].name = "geometry".to_owned();
    reserved.features[0].attributes =
        BTreeMap::from([("geometry".to_owned(), AttributeValue::U64(7))]);
    assert_rejected(&temp.path().join("reserved.parquet"), &reserved);

    let mut undeclared = base.clone();
    undeclared.features[0]
        .attributes
        .insert("extra".to_owned(), AttributeValue::Bool(true));
    assert_rejected(&temp.path().join("undeclared.parquet"), &undeclared);

    let mut mismatched = base;
    mismatched.features[0].attributes.insert(
        "class_id".to_owned(),
        AttributeValue::String("7".to_owned()),
    );
    assert_rejected(&temp.path().join("mismatched.parquet"), &mismatched);
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
        attribute_schema: vec![AttributeFieldV1 {
            name: "class_id".to_owned(),
            value_type: AttributeType::U64,
            nullable: false,
        }],
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

fn assert_rejected(path: &std::path::Path, collection: &FeatureCollectionV1) {
    assert!(matches!(
        write_geoparquet(path, collection, GeoParquetWriteOptions::default()),
        Err(spatial_io::SpatialIoError::IncompatibleAttribute { .. })
    ));
    assert!(!path.exists());
}
