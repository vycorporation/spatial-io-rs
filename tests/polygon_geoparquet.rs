#![cfg(feature = "geoparquet")]
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;
use std::fs::File;

use arrow_array::{Array, BinaryArray, StringArray};
use geoparquet::metadata::GeoParquetMetadata;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::reader::{FileReader, SerializedFileReader};
use spatial_io::{
    AttributeFieldV1, AttributeType, AttributeValue, AxisDirection, CoordinateSpace,
    FeatureCollectionV1, FeatureV1, GeoParquetWriteOptions, GeometryV1, LinearRing, MultiPolygon,
    PixelAnchor, PixelOrigin, Point2, Polygon, SpatialReference, write_geoparquet,
};

#[test]
fn writes_polygon_and_multipolygon_with_provenance_deterministically()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let first_path = temp.path().join("polygons.parquet");
    let repeated_path = temp.path().join("polygons-repeated.parquet");
    let collection = polygon_collection()?;

    let report = write_geoparquet(&first_path, &collection, GeoParquetWriteOptions::default())?;
    write_geoparquet(
        &repeated_path,
        &collection,
        GeoParquetWriteOptions::default(),
    )?;

    assert_eq!(report.feature_count, 2);
    assert_eq!(report.bbox, [0.0, 0.0, 14.0, 10.0]);
    assert_eq!(std::fs::read(&first_path)?, std::fs::read(repeated_path)?);

    let metadata = read_geo_metadata(&first_path)?;
    let column = serde_json::to_value(&metadata.columns["geometry"])?;
    let mut geometry_types = column["geometry_types"]
        .as_array()
        .expect("geometry type array")
        .iter()
        .map(|value| value.as_str().expect("geometry type string"))
        .collect::<Vec<_>>();
    geometry_types.sort_unstable();
    assert_eq!(geometry_types, ["MultiPolygon", "Polygon"]);
    assert_eq!(column["crs"], serde_json::Value::Null);
    assert!(column.get("orientation").is_none());

    let batch = read_batch(&first_path)?;
    assert_eq!(batch.num_rows(), 2);
    let feature_ids = batch
        .column_by_name("feature_id")
        .expect("feature id")
        .as_any()
        .downcast_ref::<StringArray>()
        .expect("string ids");
    let source_ids = batch
        .column_by_name("source_primitive_id")
        .expect("source id")
        .as_any()
        .downcast_ref::<StringArray>()
        .expect("string source ids");
    let group_ids = batch
        .column_by_name("group_id")
        .expect("group id")
        .as_any()
        .downcast_ref::<StringArray>()
        .expect("string group ids");
    assert_eq!(feature_ids.value(0), "polygon-feature");
    assert_eq!(feature_ids.value(1), "multipolygon-feature");
    assert_eq!(source_ids.value(0), "source-polygon");
    assert_eq!(source_ids.value(1), "source-multipolygon");
    assert_eq!(group_ids.value(0), "topology-fixture");
    assert_eq!(group_ids.value(1), "topology-fixture");

    let geometry = batch
        .column_by_name("geometry")
        .expect("geometry")
        .as_any()
        .downcast_ref::<BinaryArray>()
        .expect("binary WKB");
    assert_eq!(
        wkb::reader::read_wkb(geometry.value(0))?.geometry_type(),
        wkb::reader::GeometryType::Polygon
    );
    assert_eq!(little_endian_u32(geometry.value(0), 5), 2);
    assert_eq!(
        wkb::reader::read_wkb(geometry.value(1))?.geometry_type(),
        wkb::reader::GeometryType::MultiPolygon
    );
    assert_eq!(little_endian_u32(geometry.value(1), 5), 2);
    Ok(())
}

fn polygon_collection() -> Result<FeatureCollectionV1, Box<dyn std::error::Error>> {
    let polygon = Polygon::new(
        rectangle(0.0, 0.0, 10.0, 10.0)?,
        vec![rectangle(2.0, 2.0, 4.0, 4.0)?],
    )?;
    let multipart = MultiPolygon::new(vec![
        Polygon::new(rectangle(11.0, 0.0, 12.0, 1.0)?, vec![])?,
        Polygon::new(rectangle(13.0, 2.0, 14.0, 3.0)?, vec![])?,
    ])?;
    Ok(FeatureCollectionV1 {
        spatial_reference: SpatialReference {
            coordinate_space: CoordinateSpace::Pixel {
                origin: PixelOrigin::TopLeft,
                y_axis: AxisDirection::Down,
                anchor: PixelAnchor::Corner,
            },
            affine: None,
            raster_interpretation: None,
        },
        attribute_schema: vec![AttributeFieldV1 {
            name: "label".to_owned(),
            value_type: AttributeType::String,
            nullable: false,
        }],
        features: vec![
            feature(
                "polygon-feature",
                "source-polygon",
                GeometryV1::Polygon(polygon),
            ),
            feature(
                "multipolygon-feature",
                "source-multipolygon",
                GeometryV1::MultiPolygon(multipart),
            ),
        ],
    })
}

fn feature(id: &str, source_id: &str, geometry: GeometryV1) -> FeatureV1 {
    FeatureV1 {
        feature_id: id.to_owned(),
        source_primitive_id: source_id.to_owned(),
        geometry,
        attributes: BTreeMap::from([("label".to_owned(), AttributeValue::String(id.to_owned()))]),
        group_id: Some("topology-fixture".to_owned()),
        conversion_profile_id: Some("literal_polygon_fixture_v1".to_owned()),
        conversion_tolerance: Some(0.25),
    }
}

fn rectangle(
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
) -> Result<LinearRing, spatial_io::SpatialIoError> {
    LinearRing::new(
        [
            (xmin, ymin),
            (xmax, ymin),
            (xmax, ymax),
            (xmin, ymax),
            (xmin, ymin),
        ]
        .into_iter()
        .map(|(x, y)| Point2::new(x, y))
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

fn read_batch(
    path: &std::path::Path,
) -> Result<arrow_array::RecordBatch, Box<dyn std::error::Error>> {
    let mut reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?.build()?;
    Ok(reader.next().expect("one batch")?)
}

fn little_endian_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("four-byte WKB field"),
    )
}
