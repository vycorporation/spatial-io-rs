#[cfg(feature = "geoparquet")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeMap;

    use spatial_io::{
        AttributeFieldV1, AttributeType, AttributeValue, AxisDirection, CoordinateSpace,
        CubicBezier, CubicPath, FeatureCollectionV1, FeatureV1, FlattenOptions,
        GeoParquetWriteOptions, GeometryV1, PixelAnchor, PixelOrigin, Point2, SpatialReference,
        flatten_cubic_path, write_geoparquet,
    };

    let cubic = CubicBezier::new(
        Point2::new(0.0, 0.0)?,
        Point2::new(10.0, 0.0)?,
        Point2::new(10.0, 10.0)?,
        Point2::new(20.0, 10.0)?,
    );
    let path = CubicPath::new(vec![cubic])?;
    let derived = flatten_cubic_path(
        &path,
        vec!["curve-0".to_owned()],
        FlattenOptions::new(0.25)?,
    )?;
    let collection = FeatureCollectionV1 {
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
            name: "class".to_owned(),
            value_type: AttributeType::String,
            nullable: false,
        }],
        features: vec![FeatureV1 {
            feature_id: "feature-0".to_owned(),
            source_primitive_id: "curve-0".to_owned(),
            geometry: GeometryV1::LineString(derived.line),
            attributes: BTreeMap::from([(
                "class".to_owned(),
                AttributeValue::String("edge".to_owned()),
            )]),
            group_id: None,
            conversion_profile_id: Some(derived.profile_id.to_owned()),
            conversion_tolerance: Some(derived.tolerance),
        }],
    };
    let output = std::env::temp_dir().join("spatial-io-example.parquet");
    let report = write_geoparquet(
        &output,
        &collection,
        GeoParquetWriteOptions { overwrite: true },
    )?;
    println!(
        "wrote {} bytes to {}",
        report.byte_length,
        report.path.display()
    );
    Ok(())
}

#[cfg(not(feature = "geoparquet"))]
fn main() {
    eprintln!("run with --features geoparquet");
}
