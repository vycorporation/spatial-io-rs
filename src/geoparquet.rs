//! Optional `GeoParquet` 1.1 WKB writer.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::{
    ArrayRef, BinaryArray, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray,
    StructArray, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::file::metadata::KeyValue;
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    AttributeType, AttributeValue, CoordinateSpace, Crs, FeatureCollectionV1, GeometryV1,
    LineString, LinearRing, MultiPolygon, Polygon, SpatialIoError,
};

const RESERVED_COLUMNS: [&str; 7] = [
    "feature_id",
    "source_primitive_id",
    "group_id",
    "conversion_profile_id",
    "conversion_tolerance",
    "geometry",
    "bbox",
];

/// `GeoParquet` publication behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GeoParquetWriteOptions {
    /// Replace an existing destination atomically when true.
    pub overwrite: bool,
}

/// Deterministic report for a published spatial artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WriteReport {
    /// Published destination.
    pub path: PathBuf,
    /// Number of feature rows.
    pub feature_count: u64,
    /// Published file length.
    pub byte_length: u64,
    /// Lowercase SHA-256 digest.
    pub sha256: String,
    /// Aggregate `[xmin, ymin, xmax, ymax]`.
    pub bbox: [f64; 4],
    /// Explicit CRS identity used by the writer.
    pub crs_identity: String,
    /// Sorted conversion-profile identities present in the collection.
    pub conversion_profile_ids: Vec<String>,
}

/// Writes topology-validated `LineString`, `Polygon`, and `MultiPolygon`
/// features as `GeoParquet` 1.1 WKB.
///
/// # Errors
///
/// Returns a typed error when geometry, attributes, CRS, encoding, or atomic
/// publication fails. The destination is not reported as successful until the
/// complete artifact has been synced and attested.
pub fn write_geoparquet(
    path: impl AsRef<Path>,
    collection: &FeatureCollectionV1,
    options: GeoParquetWriteOptions,
) -> Result<WriteReport, SpatialIoError> {
    let path = path.as_ref();
    if collection.features.is_empty() {
        return Err(SpatialIoError::InvalidGeometry(
            "GeoParquet output requires at least one feature".to_owned(),
        ));
    }
    validate_feature_metadata(collection)?;
    let geometries = collection
        .features
        .iter()
        .map(|feature| match &feature.geometry {
            geometry @ (GeometryV1::LineString(_)
            | GeometryV1::Polygon(_)
            | GeometryV1::MultiPolygon(_)) => Ok(geometry),
            other => Err(SpatialIoError::UnsupportedPrimitive(format!(
                "GeoParquet writer accepts LineString, Polygon, or MultiPolygon, got {other:?}"
            ))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let row_bounds = geometries
        .iter()
        .map(|geometry| bounds(geometry))
        .collect::<Result<Vec<_>, _>>()?;
    let bbox = aggregate_bounds(&row_bounds);
    let (crs_json, crs_identity) = resolve_crs(&collection.spatial_reference.coordinate_space)?;
    let geometry_types = geometry_type_names(&geometries);
    let geo_metadata = build_geo_metadata(&crs_json, bbox, &geometry_types)?;
    let batch = build_batch(collection, &geometries, &row_bounds)?;

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| publication_error(path, error))?;
    let properties = WriterProperties::builder()
        .set_created_by("spatial-io 0.1.0".to_owned())
        .set_key_value_metadata(Some(vec![KeyValue::new(
            "geo".to_owned(),
            Some(geo_metadata),
        )]))
        .build();
    {
        let mut writer =
            ArrowWriter::try_new(temporary.as_file_mut(), batch.schema(), Some(properties))
                .map_err(|error| SpatialIoError::GeoParquet(error.to_string()))?;
        writer
            .write(&batch)
            .map_err(|error| SpatialIoError::GeoParquet(error.to_string()))?;
        writer
            .close()
            .map_err(|error| SpatialIoError::GeoParquet(error.to_string()))?;
    }
    temporary
        .as_file_mut()
        .flush()
        .map_err(|error| publication_error(path, error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| publication_error(path, error))?;
    if options.overwrite {
        temporary
            .persist(path)
            .map_err(|error| publication_error(path, error.error))?;
    } else {
        temporary
            .persist_noclobber(path)
            .map_err(|error| publication_error(path, error.error))?;
    }
    sync_parent(parent, path)?;

    let (byte_length, sha256) = attest(path)?;
    let conversion_profile_ids = collection
        .features
        .iter()
        .filter_map(|feature| feature.conversion_profile_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(WriteReport {
        path: path.to_owned(),
        feature_count: collection
            .features
            .len()
            .try_into()
            .map_err(|_| SpatialIoError::GeoParquet("feature count exceeds u64".to_owned()))?,
        byte_length,
        sha256,
        bbox,
        crs_identity,
        conversion_profile_ids,
    })
}

fn validate_feature_metadata(collection: &FeatureCollectionV1) -> Result<(), SpatialIoError> {
    let mut declared = BTreeSet::new();
    for field in &collection.attribute_schema {
        if field.name.is_empty()
            || RESERVED_COLUMNS.contains(&field.name.as_str())
            || !declared.insert(field.name.as_str())
        {
            return Err(SpatialIoError::IncompatibleAttribute {
                name: field.name.clone(),
            });
        }
    }
    for feature in &collection.features {
        if let Some(tolerance) = feature.conversion_tolerance
            && (!tolerance.is_finite() || tolerance <= 0.0)
        {
            return Err(SpatialIoError::InvalidTolerance(tolerance));
        }
        for (name, value) in &feature.attributes {
            if !declared.contains(name.as_str()) {
                return Err(SpatialIoError::IncompatibleAttribute { name: name.clone() });
            }
            if matches!(value, AttributeValue::F64(value) if !value.is_finite()) {
                return Err(SpatialIoError::NonFinite {
                    field: "attribute",
                    value: match value {
                        AttributeValue::F64(value) => *value,
                        _ => unreachable!(),
                    },
                });
            }
        }
        for field in &collection.attribute_schema {
            match feature.attributes.get(&field.name) {
                None | Some(AttributeValue::Null) if field.nullable => {}
                None | Some(AttributeValue::Null) => {
                    return Err(SpatialIoError::IncompatibleAttribute {
                        name: field.name.clone(),
                    });
                }
                Some(value) if attribute_type(value) == Some(field.value_type) => {}
                Some(_) => {
                    return Err(SpatialIoError::IncompatibleAttribute {
                        name: field.name.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn build_geo_metadata(
    crs: &serde_json::Value,
    bbox: [f64; 4],
    geometry_types: &[&str],
) -> Result<String, SpatialIoError> {
    let value = serde_json::json!({
        "version": "1.1.0",
        "primary_column": "geometry",
        "columns": {
            "geometry": {
                "encoding": "WKB",
                "geometry_types": geometry_types,
                "crs": crs,
                "edges": "planar",
                "bbox": bbox,
                "covering": {
                    "bbox": {
                        "xmin": ["bbox", "xmin"],
                        "ymin": ["bbox", "ymin"],
                        "xmax": ["bbox", "xmax"],
                        "ymax": ["bbox", "ymax"]
                    }
                }
            }
        }
    });
    let _: geoparquet::metadata::GeoParquetMetadata = serde_json::from_value(value.clone())
        .map_err(|error| SpatialIoError::GeoParquet(error.to_string()))?;
    serde_json::to_string(&value).map_err(|error| SpatialIoError::GeoParquet(error.to_string()))
}

fn resolve_crs(
    coordinate_space: &CoordinateSpace,
) -> Result<(serde_json::Value, String), SpatialIoError> {
    let crs = match coordinate_space {
        CoordinateSpace::Georeferenced { crs } => crs,
        CoordinateSpace::Pixel { .. } => {
            return Ok((serde_json::Value::Null, "pixel:null".to_owned()));
        }
        CoordinateSpace::Local { unit } => {
            return Ok((serde_json::Value::Null, format!("local:{unit}:null")));
        }
    };
    match crs {
        Crs::Unknown => Ok((serde_json::Value::Null, "unknown:null".to_owned())),
        Crs::ProjJson(value) => {
            let parsed: serde_json::Value = serde_json::from_str(value)
                .map_err(|error| SpatialIoError::InvalidProjJson(error.to_string()))?;
            if !parsed.is_object() {
                return Err(SpatialIoError::InvalidProjJson(
                    "the root value must be an object".to_owned(),
                ));
            }
            Ok((parsed, "projjson".to_owned()))
        }
        Crs::Epsg(code) => {
            let signed = i32::try_from(*code)
                .map_err(|_| SpatialIoError::UnsupportedCrs(format!("EPSG:{code} exceeds i32")))?;
            let source = epsg_utils::epsg_to_projjson(signed)
                .map_err(|error| SpatialIoError::UnsupportedCrs(format!("EPSG:{code}: {error}")))?;
            let mut parsed = serde_json::from_str(source)
                .map_err(|error| SpatialIoError::InvalidProjJson(error.to_string()))?;
            complete_projected_base_crs(&mut parsed, *code)?;
            Ok((parsed, format!("EPSG:{code}")))
        }
    }
}

fn complete_projected_base_crs(
    crs: &mut serde_json::Value,
    projected_code: u32,
) -> Result<(), SpatialIoError> {
    if crs.get("type").and_then(serde_json::Value::as_str) != Some("ProjectedCRS") {
        return Ok(());
    }
    let base = crs.get("base_crs").ok_or_else(|| {
        SpatialIoError::InvalidProjJson(format!(
            "EPSG:{projected_code} projected CRS has no base_crs"
        ))
    })?;
    if base.get("coordinate_system").is_some() {
        return Ok(());
    }
    let id = base.get("id").ok_or_else(|| {
        SpatialIoError::InvalidProjJson(format!(
            "EPSG:{projected_code} base_crs has no authority identifier"
        ))
    })?;
    if id.get("authority").and_then(serde_json::Value::as_str) != Some("EPSG") {
        return Err(SpatialIoError::InvalidProjJson(format!(
            "EPSG:{projected_code} base_crs is not EPSG-identified"
        )));
    }
    let base_code = id
        .get("code")
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| {
            SpatialIoError::InvalidProjJson(format!(
                "EPSG:{projected_code} base_crs has an invalid EPSG code"
            ))
        })?;
    let source = epsg_utils::epsg_to_projjson(base_code).map_err(|error| {
        SpatialIoError::UnsupportedCrs(format!(
            "EPSG:{projected_code} base EPSG:{base_code}: {error}"
        ))
    })?;
    let mut complete: serde_json::Value = serde_json::from_str(source)
        .map_err(|error| SpatialIoError::InvalidProjJson(error.to_string()))?;
    if complete.get("coordinate_system").is_none() {
        return Err(SpatialIoError::InvalidProjJson(format!(
            "EPSG:{projected_code} base EPSG:{base_code} has no coordinate_system"
        )));
    }
    if let Some(object) = complete.as_object_mut() {
        object.remove("$schema");
    }
    crs.as_object_mut()
        .expect("EPSG converter returned an object")
        .insert("base_crs".into(), complete);
    Ok(())
}

fn build_batch(
    collection: &FeatureCollectionV1,
    geometries: &[&GeometryV1],
    row_bounds: &[[f64; 4]],
) -> Result<RecordBatch, SpatialIoError> {
    let mut fields = Vec::new();
    let mut arrays: Vec<ArrayRef> = Vec::new();
    push_column(
        &mut fields,
        &mut arrays,
        Field::new("feature_id", DataType::Utf8, false),
        Arc::new(StringArray::from_iter_values(
            collection
                .features
                .iter()
                .map(|feature| feature.feature_id.as_str()),
        )),
    );
    push_column(
        &mut fields,
        &mut arrays,
        Field::new("source_primitive_id", DataType::Utf8, false),
        Arc::new(StringArray::from_iter_values(
            collection
                .features
                .iter()
                .map(|feature| feature.source_primitive_id.as_str()),
        )),
    );
    push_column(
        &mut fields,
        &mut arrays,
        Field::new("group_id", DataType::Utf8, true),
        Arc::new(StringArray::from(
            collection
                .features
                .iter()
                .map(|feature| feature.group_id.as_deref())
                .collect::<Vec<_>>(),
        )),
    );
    push_column(
        &mut fields,
        &mut arrays,
        Field::new("conversion_profile_id", DataType::Utf8, true),
        Arc::new(StringArray::from(
            collection
                .features
                .iter()
                .map(|feature| feature.conversion_profile_id.as_deref())
                .collect::<Vec<_>>(),
        )),
    );
    push_column(
        &mut fields,
        &mut arrays,
        Field::new("conversion_tolerance", DataType::Float64, true),
        Arc::new(Float64Array::from(
            collection
                .features
                .iter()
                .map(|feature| feature.conversion_tolerance)
                .collect::<Vec<_>>(),
        )),
    );
    append_attributes(collection, &mut fields, &mut arrays);
    let wkbs = geometries
        .iter()
        .map(|geometry| encode_wkb(geometry))
        .collect::<Result<Vec<_>, _>>()?;
    push_column(
        &mut fields,
        &mut arrays,
        Field::new("geometry", DataType::Binary, false),
        Arc::new(BinaryArray::from_iter_values(
            wkbs.iter().map(Vec::as_slice),
        )),
    );
    let bbox_fields = ["xmin", "ymin", "xmax", "ymax"]
        .into_iter()
        .map(|name| Arc::new(Field::new(name, DataType::Float64, false)))
        .collect::<Vec<_>>();
    let bbox_arrays = (0..4)
        .map(|coordinate| {
            Arc::new(Float64Array::from_iter_values(
                row_bounds.iter().map(|bounds| bounds[coordinate]),
            )) as ArrayRef
        })
        .collect::<Vec<_>>();
    let bbox = StructArray::try_new(bbox_fields.clone().into(), bbox_arrays, None)
        .map_err(|error| SpatialIoError::GeoParquet(error.to_string()))?;
    push_column(
        &mut fields,
        &mut arrays,
        Field::new("bbox", DataType::Struct(bbox_fields.into()), false),
        Arc::new(bbox),
    );
    let schema = Arc::new(Schema::new(fields));
    RecordBatch::try_new(schema, arrays)
        .map_err(|error| SpatialIoError::GeoParquet(error.to_string()))
}

fn append_attributes(
    collection: &FeatureCollectionV1,
    fields: &mut Vec<Field>,
    arrays: &mut Vec<ArrayRef>,
) {
    for field in &collection.attribute_schema {
        append_attribute_column(
            collection,
            &field.name,
            field.value_type,
            field.nullable,
            fields,
            arrays,
        );
    }
}

fn attribute_type(value: &AttributeValue) -> Option<AttributeType> {
    match value {
        AttributeValue::Null => None,
        AttributeValue::Bool(_) => Some(AttributeType::Bool),
        AttributeValue::I64(_) => Some(AttributeType::I64),
        AttributeValue::U64(_) => Some(AttributeType::U64),
        AttributeValue::F64(_) => Some(AttributeType::F64),
        AttributeValue::Bytes(_) => Some(AttributeType::Bytes),
        AttributeValue::String(_) => Some(AttributeType::String),
    }
}

fn append_attribute_column(
    collection: &FeatureCollectionV1,
    name: &str,
    scalar_type: AttributeType,
    nullable: bool,
    fields: &mut Vec<Field>,
    arrays: &mut Vec<ArrayRef>,
) {
    let values = collection
        .features
        .iter()
        .map(|feature| feature.attributes.get(name))
        .collect::<Vec<_>>();
    let (data_type, array): (DataType, ArrayRef) = match scalar_type {
        AttributeType::Bool => (
            DataType::Boolean,
            Arc::new(BooleanArray::from(
                values
                    .iter()
                    .map(|value| match value {
                        Some(AttributeValue::Bool(value)) => Some(*value),
                        Some(AttributeValue::Null) | None => None,
                        _ => unreachable!("attribute type validated"),
                    })
                    .collect::<Vec<_>>(),
            )),
        ),
        AttributeType::I64 => (
            DataType::Int64,
            Arc::new(Int64Array::from(
                values
                    .iter()
                    .map(|value| match value {
                        Some(AttributeValue::I64(value)) => Some(*value),
                        Some(AttributeValue::Null) | None => None,
                        _ => unreachable!("attribute type validated"),
                    })
                    .collect::<Vec<_>>(),
            )),
        ),
        AttributeType::U64 => (
            DataType::UInt64,
            Arc::new(UInt64Array::from(
                values
                    .iter()
                    .map(|value| match value {
                        Some(AttributeValue::U64(value)) => Some(*value),
                        Some(AttributeValue::Null) | None => None,
                        _ => unreachable!("attribute type validated"),
                    })
                    .collect::<Vec<_>>(),
            )),
        ),
        AttributeType::F64 => (
            DataType::Float64,
            Arc::new(Float64Array::from(
                values
                    .iter()
                    .map(|value| match value {
                        Some(AttributeValue::F64(value)) => Some(*value),
                        Some(AttributeValue::Null) | None => None,
                        _ => unreachable!("attribute type validated"),
                    })
                    .collect::<Vec<_>>(),
            )),
        ),
        AttributeType::Bytes => (
            DataType::Binary,
            Arc::new(BinaryArray::from(
                values
                    .iter()
                    .map(|value| match value {
                        Some(AttributeValue::Bytes(value)) => Some(value.as_slice()),
                        Some(AttributeValue::Null) | None => None,
                        _ => unreachable!("attribute type validated"),
                    })
                    .collect::<Vec<_>>(),
            )),
        ),
        AttributeType::String => (
            DataType::Utf8,
            Arc::new(StringArray::from(
                values
                    .iter()
                    .map(|value| match value {
                        Some(AttributeValue::String(value)) => Some(value.as_str()),
                        Some(AttributeValue::Null) | None => None,
                        _ => unreachable!("attribute type validated"),
                    })
                    .collect::<Vec<_>>(),
            )),
        ),
    };
    push_column(fields, arrays, Field::new(name, data_type, nullable), array);
}

fn push_column(fields: &mut Vec<Field>, arrays: &mut Vec<ArrayRef>, field: Field, array: ArrayRef) {
    fields.push(field);
    arrays.push(array);
}

fn geometry_type_names(geometries: &[&GeometryV1]) -> Vec<&'static str> {
    ["LineString", "Polygon", "MultiPolygon"]
        .into_iter()
        .filter(|candidate| {
            geometries.iter().any(|geometry| {
                matches!(
                    (*candidate, *geometry),
                    ("LineString", GeometryV1::LineString(_))
                        | ("Polygon", GeometryV1::Polygon(_))
                        | ("MultiPolygon", GeometryV1::MultiPolygon(_))
                )
            })
        })
        .collect()
}

fn encode_wkb(geometry: &GeometryV1) -> Result<Vec<u8>, SpatialIoError> {
    match geometry {
        GeometryV1::LineString(line) => encode_line_string_wkb(line),
        GeometryV1::Polygon(polygon) => encode_polygon_wkb(polygon),
        GeometryV1::MultiPolygon(multipolygon) => encode_multi_polygon_wkb(multipolygon),
        other => Err(SpatialIoError::UnsupportedPrimitive(format!(
            "GeoParquet writer accepts LineString, Polygon, or MultiPolygon, got {other:?}"
        ))),
    }
}

fn encode_line_string_wkb(line: &LineString) -> Result<Vec<u8>, SpatialIoError> {
    let geometry = to_geo_line_string(line.points());
    let mut output = Vec::with_capacity(wkb::writer::line_string_wkb_size(&geometry));
    wkb::writer::write_line_string(
        &mut output,
        &geometry,
        &wkb::writer::WriteOptions::default(),
    )
    .map_err(|error| SpatialIoError::Wkb(error.to_string()))?;
    Ok(output)
}

fn encode_polygon_wkb(polygon: &Polygon) -> Result<Vec<u8>, SpatialIoError> {
    let geometry = to_geo_polygon(polygon);
    let mut output = Vec::with_capacity(wkb::writer::polygon_wkb_size(&geometry));
    wkb::writer::write_polygon(
        &mut output,
        &geometry,
        &wkb::writer::WriteOptions::default(),
    )
    .map_err(|error| SpatialIoError::Wkb(error.to_string()))?;
    Ok(output)
}

fn encode_multi_polygon_wkb(multipolygon: &MultiPolygon) -> Result<Vec<u8>, SpatialIoError> {
    let geometry =
        geo_types::MultiPolygon::new(multipolygon.polygons().iter().map(to_geo_polygon).collect());
    let mut output = Vec::with_capacity(wkb::writer::multi_polygon_wkb_size(&geometry));
    wkb::writer::write_multi_polygon(
        &mut output,
        &geometry,
        &wkb::writer::WriteOptions::default(),
    )
    .map_err(|error| SpatialIoError::Wkb(error.to_string()))?;
    Ok(output)
}

fn to_geo_line_string(points: &[crate::Point2]) -> geo_types::LineString<f64> {
    geo_types::LineString::new(
        points
            .iter()
            .map(|point| geo_types::Coord {
                x: point.x(),
                y: point.y(),
            })
            .collect(),
    )
}

fn to_geo_polygon(polygon: &Polygon) -> geo_types::Polygon<f64> {
    geo_types::Polygon::new(
        to_geo_ring(polygon.exterior()),
        polygon.interiors().iter().map(to_geo_ring).collect(),
    )
}

fn to_geo_ring(ring: &LinearRing) -> geo_types::LineString<f64> {
    to_geo_line_string(ring.points())
}

fn bounds(geometry: &GeometryV1) -> Result<[f64; 4], SpatialIoError> {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    match geometry {
        GeometryV1::LineString(line) => extend_bounds(&mut bounds, line.points()),
        GeometryV1::Polygon(polygon) => extend_polygon_bounds(&mut bounds, polygon),
        GeometryV1::MultiPolygon(multipolygon) => {
            for polygon in multipolygon.polygons() {
                extend_polygon_bounds(&mut bounds, polygon);
            }
        }
        other => {
            return Err(SpatialIoError::UnsupportedPrimitive(format!(
                "GeoParquet writer accepts LineString, Polygon, or MultiPolygon, got {other:?}"
            )));
        }
    }
    if bounds.iter().all(|value| value.is_finite()) {
        Ok(bounds)
    } else {
        Err(SpatialIoError::InvalidGeometry(
            "geometry bounds are non-finite".to_owned(),
        ))
    }
}

fn extend_polygon_bounds(bounds: &mut [f64; 4], polygon: &Polygon) {
    extend_bounds(bounds, polygon.exterior().points());
    for interior in polygon.interiors() {
        extend_bounds(bounds, interior.points());
    }
}

fn extend_bounds(bounds: &mut [f64; 4], points: &[crate::Point2]) {
    for point in points {
        bounds[0] = bounds[0].min(point.x());
        bounds[1] = bounds[1].min(point.y());
        bounds[2] = bounds[2].max(point.x());
        bounds[3] = bounds[3].max(point.y());
    }
}

fn aggregate_bounds(row_bounds: &[[f64; 4]]) -> [f64; 4] {
    row_bounds.iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |mut aggregate, bounds| {
            aggregate[0] = aggregate[0].min(bounds[0]);
            aggregate[1] = aggregate[1].min(bounds[1]);
            aggregate[2] = aggregate[2].max(bounds[2]);
            aggregate[3] = aggregate[3].max(bounds[3]);
            aggregate
        },
    )
}

fn attest(path: &Path) -> Result<(u64, String), SpatialIoError> {
    let mut file = File::open(path).map_err(|error| publication_error(path, error))?;
    let byte_length = file
        .metadata()
        .map_err(|error| publication_error(path, error))?
        .len();
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| publication_error(path, error))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok((byte_length, hex::encode(digest.finalize())))
}

#[cfg(unix)]
fn sync_parent(parent: &Path, destination: &Path) -> Result<(), SpatialIoError> {
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| publication_error(destination, error))
}

#[cfg(not(unix))]
fn sync_parent(_parent: &Path, _destination: &Path) -> Result<(), SpatialIoError> {
    Ok(())
}

fn publication_error(path: &Path, error: impl std::fmt::Display) -> SpatialIoError {
    SpatialIoError::Publication {
        path: path.to_owned(),
        message: error.to_string(),
    }
}
