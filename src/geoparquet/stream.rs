//! Bounded iterator writer for independent `LineString` values.
use super::{
    attest, bounds, build_batch, build_geo_metadata, build_spatial_metadata, publication_error,
    resolve_crs, sync_parent, validate_feature_metadata,
};
use crate::{AttributeFieldV1, FeatureV1, SpatialReference};
use crate::{AttributeValue, FeatureCollectionV1, GeometryV1, SpatialIoError};
use parquet::{
    arrow::ArrowWriter,
    file::{metadata::KeyValue, properties::WriterProperties},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

const MAX_ROWS: u64 = 512 * 32_768;

/// Explicit ceilings for the iterator-based `LineString` writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeoParquetStreamOptions {
    /// Maximum number of source rows retained per conversion batch.
    pub batch_rows: usize,
    /// Maximum number of rows admitted in this artifact.
    pub max_features: u64,
    /// Maximum vertices admitted in a single `LineString`.
    pub max_vertices_per_feature: usize,
    /// Maximum total UTF-8/binary attribute and identity bytes per row.
    pub max_attribute_bytes_per_feature: usize,
}
impl Default for GeoParquetStreamOptions {
    fn default() -> Self {
        Self {
            batch_rows: 256,
            max_features: 10_000_000,
            max_vertices_per_feature: 65_537,
            max_attribute_bytes_per_feature: 1_048_576,
        }
    }
}
/// Attestation of a completely published bounded `LineString` artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoParquetStreamReport {
    /// Published destination.
    pub path: PathBuf,
    /// Exact row count, including zero for an empty source.
    pub feature_count: u64,
    /// Published file size.
    pub byte_length: u64,
    /// SHA-256 computed from the staged inode before publication.
    pub sha256: String,
    /// Aggregate bounds, absent when there are no rows.
    pub bbox: Option<[f64; 4]>,
    /// Explicit resolved CRS identity.
    pub crs_identity: String,
    /// Sorted conversion profiles present in the output.
    pub conversion_profile_ids: Vec<String>,
}

/// Writes a bounded iterator of independent `LineString` values without collecting the run.
///
/// Row order, typed attributes and metadata are independent of `batch_rows`.
/// Physical row groups use a fixed 512-row limit. At most one caller-sized
/// batch plus Parquet buffers is retained. Callers must bound creation of each
/// input feature too; iterator-owned source storage is outside this writer.
/// Empty input produces a typed zero-row file with no aggregate bbox.
/// The writer also caps encoded rows at 1 MiB, conversion batches at 64 MiB,
/// profiles at 16 and total rows at 16,777,216 (Parquet row-group capacity).
/// The schema is capped at 256 attributes and 64 KiB of field names. These
/// internal ceilings apply even when a caller asks for larger limits. The
/// 64 MiB admission budget is not a total RSS claim: Arrow/WKB copies, Parquet
/// row-group buffers and retained footer metadata consume additional memory;
/// footer storage scales with the bounded number of rows/groups.
///
/// The iterator may return an error for cancellation or source failure. Errors
/// at any point discard private staging. An existing destination is never
/// replaced; no successful report precedes sync and no-clobber publication.
///
/// # Errors
/// Returns resource, geometry, schema, iterator or publication errors. Supports
/// only `LineString` geometry; no implicit joins or topology conversion occur.
pub fn write_line_strings_geoparquet(
    path: impl AsRef<Path>,
    reference: &SpatialReference,
    attribute_schema: &[AttributeFieldV1],
    features: impl IntoIterator<Item = Result<FeatureV1, SpatialIoError>>,
    options: GeoParquetStreamOptions,
) -> Result<GeoParquetStreamReport, SpatialIoError> {
    validate_options(options)?;
    validate_schema_bounds(attribute_schema)?;
    let path = path.as_ref();
    let mut collection = FeatureCollectionV1 {
        spatial_reference: reference.clone(),
        attribute_schema: attribute_schema.to_vec(),
        features: Vec::new(),
    };
    validate_feature_metadata(&collection)?;
    let schema = build_batch(&collection, &[], &[])?.schema();
    let (crs, crs_identity) = resolve_crs(&reference.coordinate_space)?;
    let spatial_metadata = build_spatial_metadata(reference)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| publication_error(path, e))?;
    let properties = WriterProperties::builder()
        .set_created_by(concat!("spatial-io ", env!("CARGO_PKG_VERSION")).to_owned())
        .set_max_row_group_row_count(Some(512))
        .set_write_batch_size(1)
        .build();
    let mut count = 0_u64;
    let mut buffered_bytes = 0_usize;
    let mut bbox: Option<[f64; 4]> = None;
    let mut profiles = BTreeSet::new();
    {
        let mut writer = ArrowWriter::try_new(temporary.as_file_mut(), schema, Some(properties))
            .map_err(|e| parquet_error(&e))?;
        for feature in features {
            let feature = feature?;
            if count == options.max_features {
                return Err(limit(
                    "features",
                    usize::try_from(options.max_features).unwrap_or(usize::MAX),
                ));
            }
            let row_bytes = feature_bytes(&feature, options, attribute_schema.len())?;
            if buffered_bytes + row_bytes > 67_108_864 {
                write_batch(&mut writer, &collection)?;
                collection.features.clear();
                buffered_bytes = 0;
            }
            buffered_bytes += row_bytes;
            if let Some(profile) = &feature.conversion_profile_id {
                profiles.insert(profile.clone());
                if profiles.len() > 16 {
                    return Err(limit("conversion profiles", 16));
                }
            }
            let row = bounds(&feature.geometry)?;
            bbox = Some(bbox.map_or(row, |b| {
                [
                    b[0].min(row[0]),
                    b[1].min(row[1]),
                    b[2].max(row[2]),
                    b[3].max(row[3]),
                ]
            }));
            count += 1;
            collection.features.push(feature);
            if collection.features.len() == options.batch_rows {
                write_batch(&mut writer, &collection)?;
                collection.features.clear();
                buffered_bytes = 0;
            }
        }
        if !collection.features.is_empty() {
            write_batch(&mut writer, &collection)?;
        }
        writer.append_key_value_metadata(KeyValue::new(
            "geo".to_owned(),
            Some(stream_geo_metadata(&crs, bbox)?),
        ));
        writer.append_key_value_metadata(KeyValue::new(
            "spatial_io".to_owned(),
            Some(spatial_metadata),
        ));
        writer.close().map_err(|e| parquet_error(&e))?;
    }
    let (byte_length, sha256) = publish_staged(temporary, path, parent)?;
    Ok(GeoParquetStreamReport {
        path: path.to_owned(),
        feature_count: count,
        byte_length,
        sha256,
        bbox,
        crs_identity,
        conversion_profile_ids: profiles.into_iter().collect(),
    })
}
fn parquet_error(e: &parquet::errors::ParquetError) -> SpatialIoError {
    SpatialIoError::GeoParquet(e.to_string())
}
fn limit(resource: &'static str, limit: usize) -> SpatialIoError {
    SpatialIoError::ResourceLimit { resource, limit }
}
fn write_batch(
    writer: &mut ArrowWriter<&mut File>,
    collection: &FeatureCollectionV1,
) -> Result<(), SpatialIoError> {
    validate_feature_metadata(collection)?;
    let geometries = collection
        .features
        .iter()
        .map(|f| &f.geometry)
        .collect::<Vec<_>>();
    let row_bounds = geometries
        .iter()
        .map(|g| bounds(g))
        .collect::<Result<Vec<_>, _>>()?;
    writer
        .write(&build_batch(collection, &geometries, &row_bounds)?)
        .map_err(|e| parquet_error(&e))
}
fn validate_row_size(feature: &FeatureV1, max: usize) -> Result<usize, SpatialIoError> {
    let mut lengths = std::iter::once(feature.feature_id.len())
        .chain(std::iter::once(feature.source_primitive_id.len()))
        .chain(feature.group_id.as_ref().map(String::len))
        .chain(feature.conversion_profile_id.as_ref().map(String::len))
        .chain(feature.attributes.iter().map(|(name, value)| {
            name.len().saturating_add(match value {
                AttributeValue::Bytes(v) => v.len(),
                AttributeValue::String(v) => v.len(),
                _ => 8,
            })
        }));
    let total = lengths.try_fold(0_usize, usize::checked_add);
    if total.is_none_or(|n| n > max) {
        return Err(limit("attribute bytes per feature", max));
    }
    total.ok_or_else(|| limit("attribute bytes per feature", max))
}

fn stream_geo_metadata(
    crs: &serde_json::Value,
    bbox: Option<[f64; 4]>,
) -> Result<String, SpatialIoError> {
    let mut geo: serde_json::Value = serde_json::from_str(&build_geo_metadata(
        crs,
        bbox.unwrap_or([0.; 4]),
        &["LineString"],
    )?)
    .map_err(|e| SpatialIoError::GeoParquet(e.to_string()))?;
    if bbox.is_none()
        && let Some(object) = geo["columns"]["geometry"].as_object_mut()
    {
        object.remove("bbox");
    }
    Ok(geo.to_string())
}
fn publish_staged(
    mut temporary: tempfile::NamedTempFile,
    path: &Path,
    parent: &Path,
) -> Result<(u64, String), SpatialIoError> {
    temporary
        .as_file_mut()
        .flush()
        .map_err(|e| publication_error(path, e))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| publication_error(path, e))?;
    let attestation = attest(temporary.as_file_mut(), path)?;
    temporary
        .persist_noclobber(path)
        .map_err(|e| publication_error(path, e.error))?;
    sync_parent(parent, path)?;
    Ok(attestation)
}

fn validate_options(options: GeoParquetStreamOptions) -> Result<(), SpatialIoError> {
    if options.batch_rows == 0 || options.max_vertices_per_feature < 2 {
        return Err(SpatialIoError::InvalidGeometry(
            "batch_rows must be positive and max_vertices_per_feature at least two".into(),
        ));
    }

    if options.max_features > MAX_ROWS {
        return Err(limit(
            "configured features exceed Parquet row-group capacity",
            16_777_216,
        ));
    }
    Ok(())
}

fn feature_bytes(
    feature: &FeatureV1,
    options: GeoParquetStreamOptions,
    schema_fields: usize,
) -> Result<usize, SpatialIoError> {
    let GeometryV1::LineString(line) = &feature.geometry else {
        return Err(SpatialIoError::UnsupportedPrimitive(
            "bounded writer requires independent LineStrings".into(),
        ));
    };
    if line.points().len() > options.max_vertices_per_feature {
        return Err(limit(
            "vertices per feature",
            options.max_vertices_per_feature,
        ));
    }
    let attribute_bytes = validate_row_size(feature, options.max_attribute_bytes_per_feature)?;
    let row_bytes = line
        .points()
        .len()
        .checked_mul(16)
        .and_then(|n| n.checked_add(9))
        .and_then(|n| n.checked_add(attribute_bytes))
        .and_then(|n| n.checked_add(schema_fields * 16))
        .ok_or_else(|| limit("encoded feature bytes", 1_048_576))?;
    // Keep every 32-bit Arrow offset and the complete batch far below i32::MAX.
    if row_bytes > 1_048_576 {
        return Err(limit("encoded feature bytes", 1_048_576));
    }
    Ok(row_bytes)
}

fn validate_schema_bounds(schema: &[AttributeFieldV1]) -> Result<(), SpatialIoError> {
    if schema.len() > 256 {
        return Err(limit("attribute schema fields", 256));
    }
    let name_bytes = schema
        .iter()
        .try_fold(0_usize, |sum, f| sum.checked_add(f.name.len()));
    if name_bytes.is_none_or(|n| n > 65_536) {
        return Err(limit("attribute schema name bytes", 65_536));
    }
    Ok(())
}
