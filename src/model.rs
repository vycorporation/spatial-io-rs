//! Crate-owned geometry, feature, and attribute contracts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::SpatialIoError;

/// A finite two-dimensional coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    x: f64,
    y: f64,
}

impl Point2 {
    /// Creates a point after rejecting non-finite coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialIoError::NonFinite`] when either coordinate is not
    /// finite.
    pub fn new(x: f64, y: f64) -> Result<Self, SpatialIoError> {
        validate_finite("x", x)?;
        validate_finite("y", y)?;
        Ok(Self { x, y })
    }

    /// Returns the x coordinate.
    #[must_use]
    pub const fn x(self) -> f64 {
        self.x
    }

    /// Returns the y coordinate.
    #[must_use]
    pub const fn y(self) -> f64 {
        self.y
    }
}

/// One cubic Bézier segment with endpoints `p0` and `p3`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CubicBezier {
    /// Start point.
    pub p0: Point2,
    /// First control point.
    pub p1: Point2,
    /// Second control point.
    pub p2: Point2,
    /// End point.
    pub p3: Point2,
}

impl CubicBezier {
    /// Creates a cubic from four already validated finite points.
    #[must_use]
    pub const fn new(p0: Point2, p1: Point2, p2: Point2, p3: Point2) -> Self {
        Self { p0, p1, p2, p3 }
    }
}

/// An ordered, connected sequence of cubic Bézier segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CubicPath {
    segments: Vec<CubicBezier>,
}

impl CubicPath {
    /// Creates a non-empty path and verifies exact endpoint connectivity.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialIoError::InvalidGeometry`] for an empty or
    /// disconnected path.
    pub fn new(segments: Vec<CubicBezier>) -> Result<Self, SpatialIoError> {
        if segments.is_empty() {
            return Err(SpatialIoError::InvalidGeometry(
                "a cubic path must contain at least one segment".to_owned(),
            ));
        }
        if segments.windows(2).any(|pair| pair[0].p3 != pair[1].p0) {
            return Err(SpatialIoError::InvalidGeometry(
                "cubic path segments must share exact endpoints".to_owned(),
            ));
        }
        Ok(Self { segments })
    }

    /// Returns the ordered segments.
    #[must_use]
    pub fn segments(&self) -> &[CubicBezier] {
        &self.segments
    }
}

/// A `LineString` containing at least two finite points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineString {
    points: Vec<Point2>,
}

impl LineString {
    /// Creates a `LineString` after validating its minimum size.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialIoError::InvalidGeometry`] when fewer than two points
    /// are supplied.
    pub fn new(points: Vec<Point2>) -> Result<Self, SpatialIoError> {
        if points.len() < 2 {
            return Err(SpatialIoError::InvalidGeometry(
                "a LineString must contain at least two points".to_owned(),
            ));
        }
        Ok(Self { points })
    }

    /// Returns the ordered points.
    #[must_use]
    pub fn points(&self) -> &[Point2] {
        &self.points
    }

    /// Applies a point mapping while preserving order.
    ///
    /// # Errors
    ///
    /// Returns the first mapping error or a resulting geometry error.
    pub fn try_map(
        &self,
        mut map: impl FnMut(Point2) -> Result<Point2, SpatialIoError>,
    ) -> Result<Self, SpatialIoError> {
        Self::new(
            self.points
                .iter()
                .copied()
                .map(&mut map)
                .collect::<Result<Vec<_>, _>>()?,
        )
    }
}

/// Initial, versioned geometry carrier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GeometryV1 {
    /// One point.
    Point(Point2),
    /// One `LineString`.
    LineString(LineString),
    /// Multiple independent `LineString` values.
    MultiLineString(Vec<LineString>),
    /// Exact source cubic geometry requiring a conversion profile for WKB.
    CubicPath(CubicPath),
    /// One topology-validated polygon with explicit shell and holes.
    Polygon(crate::Polygon),
    /// Multiple topology-validated polygons with explicit grouping.
    MultiPolygon(crate::MultiPolygon),
}

/// Supported deterministic scalar attribute values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AttributeValue {
    /// Explicit null.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Signed 64-bit integer.
    I64(i64),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Floating-point value; writers reject non-finite values.
    F64(f64),
    /// Binary bytes.
    Bytes(Vec<u8>),
    /// UTF-8 text.
    String(String),
}

/// Declared logical type for one scalar attribute column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeType {
    /// Boolean.
    Bool,
    /// Signed 64-bit integer.
    I64,
    /// Unsigned 64-bit integer.
    U64,
    /// 64-bit floating point.
    F64,
    /// Binary bytes.
    Bytes,
    /// UTF-8 text.
    String,
}

/// Ordered schema declaration for one feature attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeFieldV1 {
    /// Stable non-empty column name.
    pub name: String,
    /// Declared scalar type, including when every value is null.
    pub value_type: AttributeType,
    /// Whether null or missing values are permitted.
    pub nullable: bool,
}

/// One stable feature and its source provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureV1 {
    /// Stable feature identity.
    pub feature_id: String,
    /// Stable source primitive identity.
    pub source_primitive_id: String,
    /// Geometry in the collection coordinate space.
    pub geometry: GeometryV1,
    /// Typed scalar attributes in deterministic name order.
    pub attributes: BTreeMap<String, AttributeValue>,
    /// Optional producer-defined grouping identity.
    pub group_id: Option<String>,
    /// Conversion profile that produced a derived geometry.
    pub conversion_profile_id: Option<String>,
    /// Effective conversion tolerance in collection coordinate units.
    pub conversion_tolerance: Option<f64>,
}

/// An ordered feature collection sharing one spatial reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureCollectionV1 {
    /// Spatial reference shared by every feature.
    pub spatial_reference: crate::SpatialReference,
    /// Explicit attribute columns in deterministic output order.
    pub attribute_schema: Vec<AttributeFieldV1>,
    /// Features in stable output order.
    pub features: Vec<FeatureV1>,
}

pub(crate) fn validate_finite(field: &'static str, value: f64) -> Result<(), SpatialIoError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(SpatialIoError::NonFinite { field, value })
    }
}
