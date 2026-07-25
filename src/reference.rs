//! Coordinate-space, affine, and CRS contracts.

use serde::{Deserialize, Serialize};

use crate::{Point2, SpatialIoError};

/// Origin convention for pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelOrigin {
    /// Origin at the top-left of the image.
    TopLeft,
    /// Origin at the bottom-left of the image.
    BottomLeft,
}

/// Direction in which y coordinates increase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisDirection {
    /// Y increases downwards.
    Down,
    /// Y increases upwards.
    Up,
}

/// Meaning of an integer-valued pixel coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelAnchor {
    /// Integer coordinates denote pixel grid corners.
    Corner,
    /// Integer coordinates denote pixel centers.
    Center,
}

/// `GeoTIFF` raster-space interpretation retained from source metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RasterInterpretation {
    /// Pixels represent areas.
    PixelIsArea,
    /// Pixels represent sample points.
    PixelIsPoint,
}

/// Crate-owned coordinate reference system identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Crs {
    /// EPSG authority code.
    Epsg(u32),
    /// Caller-provided, validated PROJJSON object.
    ProjJson(String),
    /// Explicitly unknown or local CRS.
    Unknown,
}

impl Crs {
    /// Creates a nonzero EPSG authority identity.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialIoError::UnsupportedCrs`] for code zero.
    pub fn epsg(code: u32) -> Result<Self, SpatialIoError> {
        if code == 0 {
            return Err(SpatialIoError::UnsupportedCrs(
                "EPSG code must be greater than zero".to_owned(),
            ));
        }
        Ok(Self::Epsg(code))
    }

    /// Validates that a string contains a JSON object before retaining it.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialIoError::InvalidProjJson`] when parsing fails or the
    /// root value is not an object.
    pub fn projjson(value: impl Into<String>) -> Result<Self, SpatialIoError> {
        let value = value.into();
        let parsed: serde_json::Value = serde_json::from_str(&value)
            .map_err(|error| SpatialIoError::InvalidProjJson(error.to_string()))?;
        if !parsed.is_object() {
            return Err(SpatialIoError::InvalidProjJson(
                "the root value must be an object".to_owned(),
            ));
        }
        Ok(Self::ProjJson(value))
    }
}

/// Coordinate-space meaning for a feature collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CoordinateSpace {
    /// Image or canvas pixel coordinates.
    Pixel {
        /// Pixel origin.
        origin: PixelOrigin,
        /// Y-axis direction.
        y_axis: AxisDirection,
        /// Pixel anchoring used by the geometry producer.
        anchor: PixelAnchor,
    },
    /// Cartesian coordinates with a caller-defined unit and no assigned CRS.
    Local {
        /// Unit name, such as `millimetre` or `canvas_unit`.
        unit: String,
    },
    /// World coordinates with an explicit CRS.
    Georeferenced {
        /// Coordinate reference system.
        crs: Crs,
    },
}

/// Exact six-coefficient affine mapping input `(x, y)` to output `(x, y)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Affine2D {
    /// Output x origin.
    pub origin_x: f64,
    /// X contribution to output x.
    pub x_scale: f64,
    /// Y contribution to output x.
    pub x_skew: f64,
    /// Output y origin.
    pub origin_y: f64,
    /// X contribution to output y.
    pub y_skew: f64,
    /// Y contribution to output y.
    pub y_scale: f64,
}

impl Affine2D {
    /// Creates a finite, invertible affine transform.
    ///
    /// # Errors
    ///
    /// Returns an error for non-finite coefficients or a singular linear
    /// component.
    pub fn new(
        origin_x: f64,
        x_scale: f64,
        x_skew: f64,
        origin_y: f64,
        y_skew: f64,
        y_scale: f64,
    ) -> Result<Self, SpatialIoError> {
        for (field, value) in [
            ("origin_x", origin_x),
            ("x_scale", x_scale),
            ("x_skew", x_skew),
            ("origin_y", origin_y),
            ("y_skew", y_skew),
            ("y_scale", y_scale),
        ] {
            crate::model::validate_finite(field, value)?;
        }
        let determinant = x_scale.mul_add(y_scale, -(x_skew * y_skew));
        if !determinant.is_finite() || determinant.abs() <= f64::EPSILON {
            return Err(SpatialIoError::InvalidAffine(
                "linear component is singular".to_owned(),
            ));
        }
        Ok(Self {
            origin_x,
            x_scale,
            x_skew,
            origin_y,
            y_skew,
            y_scale,
        })
    }

    /// Applies the affine after honoring the input pixel anchor.
    ///
    /// # Errors
    ///
    /// Returns an error if the transformed coordinate is non-finite.
    pub fn transform(self, point: Point2, anchor: PixelAnchor) -> Result<Point2, SpatialIoError> {
        let offset = match anchor {
            PixelAnchor::Corner => 0.0,
            PixelAnchor::Center => 0.5,
        };
        let x = point.x() + offset;
        let y = point.y() + offset;
        Point2::new(
            self.origin_x + self.x_scale.mul_add(x, self.x_skew * y),
            self.origin_y + self.y_skew.mul_add(x, self.y_scale * y),
        )
    }
}

/// Coordinate-space declaration plus optional input-to-world affine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialReference {
    /// Meaning of geometry coordinates.
    pub coordinate_space: CoordinateSpace,
    /// Optional affine to another declared space.
    pub affine: Option<Affine2D>,
    /// Optional source raster interpretation.
    pub raster_interpretation: Option<RasterInterpretation>,
}

/// Applies one exact affine to every `LineString` point using the declared anchor.
///
/// # Errors
///
/// Returns the first non-finite transformed coordinate or resulting geometry
/// error.
pub fn transform_line_string(
    line: &crate::LineString,
    affine: Affine2D,
    anchor: PixelAnchor,
) -> Result<crate::LineString, SpatialIoError> {
    line.try_map(|point| affine.transform(point, anchor))
}
