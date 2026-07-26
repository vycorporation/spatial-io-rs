#[cfg(feature = "geoparquet")]
mod enabled {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde::Serialize;
    use spatial_io::{
        AttributeFieldV1, AttributeType, AttributeValue, AxisDirection, CoordinateSpace, Crs,
        FeatureCollectionV1, FeatureV1, GeoParquetWriteOptions, GeometryV1, LineString,
        PixelAnchor, PixelOrigin, Point2, SpatialReference, WriteReport, write_geoparquet,
    };

    #[derive(Serialize)]
    struct Manifest<'a> {
        schema: &'a str,
        license: &'a str,
        generator: &'a str,
        geometry_encoding: &'a str,
        fixtures: Vec<FixtureRecord>,
    }

    #[derive(Serialize)]
    struct FixtureRecord {
        file: &'static str,
        coordinate_space: &'static str,
        expected_crs: Option<&'static str>,
        geometry_types: [&'static str; 1],
        feature_count: u64,
        bbox: [f64; 4],
        byte_length: u64,
        sha256: String,
    }

    type Case = (
        &'static str,
        &'static str,
        Option<&'static str>,
        FeatureCollectionV1,
    );

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let output = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .ok_or("usage: generate_interoperability_fixtures OUTPUT_DIRECTORY")?;
        fs::create_dir_all(&output)?;

        let cases = [
            (
                "pixel.parquet",
                "pixel_top_left_y_down_corner",
                None,
                pixel_collection()?,
            ),
            (
                "local.parquet",
                "local_millimetre",
                None,
                local_collection()?,
            ),
            (
                "epsg-32618.parquet",
                "georeferenced",
                Some("EPSG:32618"),
                utm_collection()?,
            ),
        ];
        let fixtures = cases
            .into_iter()
            .map(|case| write_case(&output, case))
            .collect::<Result<Vec<_>, _>>()?;
        write_manifest(&output, fixtures)
    }

    fn pixel_collection() -> Result<FeatureCollectionV1, spatial_io::SpatialIoError> {
        Ok(collection(
            CoordinateSpace::Pixel {
                origin: PixelOrigin::TopLeft,
                y_axis: AxisDirection::Down,
                anchor: PixelAnchor::Corner,
            },
            vec![
                feature(
                    "pixel-0",
                    &[(0.0, 0.0), (10.0, 5.0), (20.0, 0.0)],
                    1,
                    Some(0.9),
                )?,
                feature("pixel-1", &[(3.0, 10.0), (8.0, 14.0)], 2, None)?,
            ],
        ))
    }

    fn local_collection() -> Result<FeatureCollectionV1, spatial_io::SpatialIoError> {
        Ok(collection(
            CoordinateSpace::Local {
                unit: "millimetre".to_owned(),
            },
            vec![
                feature(
                    "local-0",
                    &[(-2.5, 1.0), (0.0, 4.5), (3.0, 2.0)],
                    1,
                    Some(0.75),
                )?,
                feature("local-1", &[(1.0, -3.0), (5.5, -1.0)], 2, None)?,
            ],
        ))
    }

    fn utm_collection() -> Result<FeatureCollectionV1, spatial_io::SpatialIoError> {
        Ok(collection(
            CoordinateSpace::Georeferenced {
                crs: Crs::epsg(32_618)?,
            },
            vec![
                feature(
                    "utm-0",
                    &[
                        (500_000.0, 4_400_000.0),
                        (500_010.0, 4_400_020.0),
                        (500_025.0, 4_400_015.0),
                    ],
                    1,
                    Some(0.95),
                )?,
                feature(
                    "utm-1",
                    &[(499_990.0, 4_399_980.0), (500_005.0, 4_399_990.0)],
                    2,
                    None,
                )?,
            ],
        ))
    }

    fn collection(
        coordinate_space: CoordinateSpace,
        features: Vec<FeatureV1>,
    ) -> FeatureCollectionV1 {
        FeatureCollectionV1 {
            spatial_reference: SpatialReference {
                coordinate_space,
                affine: None,
                raster_interpretation: None,
            },
            attribute_schema: vec![
                field("class_id", AttributeType::U64, false),
                field("label", AttributeType::String, false),
                field("score", AttributeType::F64, true),
                field("visible", AttributeType::Bool, false),
            ],
            features,
        }
    }

    fn field(name: &str, value_type: AttributeType, nullable: bool) -> AttributeFieldV1 {
        AttributeFieldV1 {
            name: name.to_owned(),
            value_type,
            nullable,
        }
    }

    fn feature(
        id: &str,
        points: &[(f64, f64)],
        class_id: u64,
        score: Option<f64>,
    ) -> Result<FeatureV1, spatial_io::SpatialIoError> {
        Ok(FeatureV1 {
            feature_id: id.to_owned(),
            source_primitive_id: format!("source-{id}"),
            geometry: GeometryV1::LineString(line(points)?),
            attributes: BTreeMap::from([
                ("class_id".to_owned(), AttributeValue::U64(class_id)),
                (
                    "label".to_owned(),
                    AttributeValue::String(format!("class-{class_id}")),
                ),
                (
                    "score".to_owned(),
                    score.map_or(AttributeValue::Null, AttributeValue::F64),
                ),
                ("visible".to_owned(), AttributeValue::Bool(true)),
            ]),
            group_id: Some("interoperability-fixture-v1".to_owned()),
            conversion_profile_id: Some("literal_linestring_fixture_v1".to_owned()),
            conversion_tolerance: None,
        })
    }

    fn line(points: &[(f64, f64)]) -> Result<LineString, spatial_io::SpatialIoError> {
        LineString::new(
            points
                .iter()
                .map(|&(x, y)| Point2::new(x, y))
                .collect::<Result<Vec<_>, _>>()?,
        )
    }

    fn write_case(output: &Path, case: Case) -> Result<FixtureRecord, spatial_io::SpatialIoError> {
        let (file, coordinate_space, expected_crs, collection) = case;
        let report = write_geoparquet(
            output.join(file),
            &collection,
            GeoParquetWriteOptions { overwrite: true },
        )?;
        Ok(record(file, coordinate_space, expected_crs, report))
    }

    fn record(
        file: &'static str,
        coordinate_space: &'static str,
        expected_crs: Option<&'static str>,
        report: WriteReport,
    ) -> FixtureRecord {
        FixtureRecord {
            file,
            coordinate_space,
            expected_crs,
            geometry_types: ["LineString"],
            feature_count: report.feature_count,
            bbox: report.bbox,
            byte_length: report.byte_length,
            sha256: report.sha256,
        }
    }

    fn write_manifest(
        output: &Path,
        fixtures: Vec<FixtureRecord>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest = Manifest {
            schema: "spatial_io_interoperability_fixture_matrix_v1",
            license: "MIT OR Apache-2.0",
            generator: "examples/generate_interoperability_fixtures.rs",
            geometry_encoding: "GeoParquet 1.1 WKB",
            fixtures,
        };
        let mut bytes = serde_json::to_vec_pretty(&manifest)?;
        bytes.push(b'\n');
        fs::write(output.join("manifest.json"), bytes)?;
        Ok(())
    }
}

#[cfg(feature = "geoparquet")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    enabled::run()
}

#[cfg(not(feature = "geoparquet"))]
fn main() {
    eprintln!("run with --features geoparquet");
}
