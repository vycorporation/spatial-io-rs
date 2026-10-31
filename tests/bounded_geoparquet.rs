#![cfg(feature = "geoparquet")]
use arrow_array::{Array, Float64Array, Int64Array};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use spatial_io::*;
use std::{collections::BTreeMap, fs::File};
fn reference() -> SpatialReference {
    SpatialReference {
        coordinate_space: CoordinateSpace::Local {
            unit: "pixel".into(),
        },
        affine: None,
        raster_interpretation: None,
    }
}
fn fields() -> Vec<AttributeFieldV1> {
    vec![
        AttributeFieldV1 {
            name: "id".into(),
            value_type: AttributeType::I64,
            nullable: false,
        },
        AttributeFieldV1 {
            name: "unavailable".into(),
            value_type: AttributeType::F64,
            nullable: true,
        },
    ]
}
fn feature(i: i64) -> FeatureV1 {
    FeatureV1 {
        feature_id: format!("row:{i}"),
        source_primitive_id: i.to_string(),
        group_id: None,
        conversion_profile_id: Some(UNIFORM_INTERVALS_V1.into()),
        conversion_tolerance: None,
        geometry: GeometryV1::LineString(
            LineString::new(vec![
                Point2::new(f64::from(i32::try_from(i).unwrap()), 0.).unwrap(),
                Point2::new(f64::from(i32::try_from(i).unwrap()), 1.).unwrap(),
            ])
            .unwrap(),
        ),
        attributes: BTreeMap::from([
            ("id".into(), AttributeValue::I64(i)),
            ("unavailable".into(), AttributeValue::Null),
        ]),
    }
}
#[test]
fn chunks_preserve_typed_rows_null_schema_and_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let mut snapshots = Vec::new();
    for batch_rows in [1, 3, 32] {
        let path = temp.path().join(format!("{batch_rows}.parquet"));
        let options = GeoParquetStreamOptions {
            batch_rows,
            ..GeoParquetStreamOptions::default()
        };
        let report = write_line_strings_geoparquet(
            &path,
            &reference(),
            &fields(),
            (0..521).map(|i| Ok(feature(i))),
            options,
        )
        .unwrap();
        assert_eq!(report.feature_count, 521);
        assert_eq!(report.bbox, Some([0., 0., 520., 1.]));
        let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(&path).unwrap()).unwrap();
        let checksum = report.sha256.clone();
        let metadata = builder
            .metadata()
            .file_metadata()
            .key_value_metadata()
            .unwrap()
            .clone();
        let mut ids = Vec::new();
        for batch in builder.build().unwrap() {
            let b = batch.unwrap();
            let id = b
                .column_by_name("id")
                .unwrap()
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap();
            ids.extend(id.values().iter().copied());
            let absent = b
                .column_by_name("unavailable")
                .unwrap()
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap();
            assert_eq!(absent.null_count(), b.num_rows());
        }
        snapshots.push((ids, metadata, checksum));
    }
    assert!(snapshots.windows(2).all(|w| w[0] == w[1]));
}
#[test]
fn empty_output_and_late_error_are_truthful_and_atomic() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("lines.parquet");
    let empty = write_line_strings_geoparquet(
        &path,
        &reference(),
        &fields(),
        std::iter::empty(),
        GeoParquetStreamOptions::default(),
    )
    .unwrap();
    assert_eq!(empty.feature_count, 0);
    assert_eq!(empty.bbox, None);
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(&path).unwrap()).unwrap();
    assert_eq!(
        builder
            .schema()
            .field_with_name("unavailable")
            .unwrap()
            .data_type(),
        &arrow_schema::DataType::Float64
    );
    let kv = builder
        .metadata()
        .file_metadata()
        .key_value_metadata()
        .unwrap();
    let geo: serde_json::Value = serde_json::from_str(
        kv.iter()
            .find(|x| x.key == "geo")
            .unwrap()
            .value
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    assert!(geo["columns"]["geometry"].get("bbox").is_none());
    assert!(geo["columns"]["geometry"]["crs"].is_null());
    assert_eq!(
        builder
            .build()
            .unwrap()
            .map(|b| b.unwrap().num_rows())
            .sum::<usize>(),
        0
    );
    let original = std::fs::read(&path).unwrap();
    assert!(
        write_line_strings_geoparquet(
            &path,
            &reference(),
            &fields(),
            [Ok(feature(0))],
            GeoParquetStreamOptions::default()
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let fail = temp.path().join("failed.parquet");
    let options = GeoParquetStreamOptions {
        batch_rows: 1,
        ..GeoParquetStreamOptions::default()
    };
    assert!(
        write_line_strings_geoparquet(
            &fail,
            &reference(),
            &fields(),
            (0..521)
                .map(|i| Ok(feature(i)))
                .chain(std::iter::once(Err(SpatialIoError::Cancelled))),
            options
        )
        .is_err()
    );
    assert!(!fail.exists());
    let limited = temp.path().join("limited.parquet");
    let options = GeoParquetStreamOptions {
        max_features: 1,
        ..GeoParquetStreamOptions::default()
    };
    assert!(matches!(
        write_line_strings_geoparquet(
            &limited,
            &reference(),
            &fields(),
            [Ok(feature(0)), Ok(feature(1))],
            options
        ),
        Err(SpatialIoError::ResourceLimit { .. })
    ));
    assert!(!limited.exists());
}

#[test]
fn physical_row_group_capacity_is_checked_before_iteration() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("over-capacity.parquet");
    let options = GeoParquetStreamOptions {
        max_features: 16_777_217,
        ..GeoParquetStreamOptions::default()
    };
    let never = std::iter::from_fn(|| -> Option<Result<FeatureV1, SpatialIoError>> {
        panic!("invalid admission must not pull source rows")
    });
    assert!(matches!(
        write_line_strings_geoparquet(&path, &reference(), &fields(), never, options),
        Err(SpatialIoError::ResourceLimit { .. })
    ));
    assert!(!path.exists());
}

#[test]
fn dictionary_thresholds_are_independent_of_source_chunk_size() {
    let tmp = tempfile::tempdir().unwrap();
    let mut hashes = Vec::new();
    let mut schema = fields();
    schema.push(AttributeFieldV1 {
        name: "payload".into(),
        value_type: AttributeType::String,
        nullable: false,
    });
    for batch_rows in [1, 32] {
        let rows = (0..32).map(|i| {
            let mut f = feature(i);
            f.attributes.insert(
                "payload".into(),
                AttributeValue::String(format!("{i:06}{}", "x".repeat(100_000))),
            );
            Ok(f)
        });
        let report = write_line_strings_geoparquet(
            tmp.path().join(format!("{batch_rows}.parquet")),
            &reference(),
            &schema,
            rows,
            GeoParquetStreamOptions {
                batch_rows,
                ..GeoParquetStreamOptions::default()
            },
        )
        .unwrap();
        hashes.push(report.sha256);
    }
    assert_eq!(hashes[0], hashes[1]);
}
#[test]
fn encoded_feature_bytes_are_checked_before_arrow_offsets() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("too-wide.parquet");
    let mut schema = fields();
    schema.push(AttributeFieldV1 {
        name: "payload".into(),
        value_type: AttributeType::String,
        nullable: false,
    });
    let mut f = feature(0);
    f.attributes.insert(
        "payload".into(),
        AttributeValue::String("x".repeat(1_048_576)),
    );
    let options = GeoParquetStreamOptions {
        max_attribute_bytes_per_feature: 2_097_152,
        ..GeoParquetStreamOptions::default()
    };
    assert!(matches!(
        write_line_strings_geoparquet(&path, &reference(), &schema, [Ok(f)], options),
        Err(SpatialIoError::ResourceLimit {
            resource: "encoded feature bytes",
            ..
        })
    ));
    assert!(!path.exists());
}

#[test]
fn schema_width_is_bounded_even_with_empty_nullable_data() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("wide.parquet");
    let fields = (0..257)
        .map(|i| AttributeFieldV1 {
            name: format!("a{i}"),
            value_type: AttributeType::F64,
            nullable: true,
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        write_line_strings_geoparquet(
            &path,
            &reference(),
            &fields,
            std::iter::empty(),
            GeoParquetStreamOptions::default()
        ),
        Err(SpatialIoError::ResourceLimit {
            resource: "attribute schema fields",
            ..
        })
    ));
    assert!(!path.exists());
}
