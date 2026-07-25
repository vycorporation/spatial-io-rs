//! Error contract.

use std::path::PathBuf;

/// Errors returned by spatial conversion and format adapters.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SpatialIoError {
    /// A coordinate or scalar was not finite.
    #[error("invalid non-finite {field} value: {value}")]
    NonFinite {
        /// Field containing the value.
        field: &'static str,
        /// Invalid value.
        value: f64,
    },
    /// An approximation tolerance was not finite and positive.
    #[error("flatten tolerance must be finite and greater than zero, got {0}")]
    InvalidTolerance(f64),
    /// A geometry did not contain the minimum required coordinates.
    #[error("invalid geometry: {0}")]
    InvalidGeometry(String),
    /// A source primitive cannot be converted by the selected profile.
    #[error("unsupported primitive: {0}")]
    UnsupportedPrimitive(String),
    /// Attribute values for a named column used incompatible scalar types.
    #[error("attribute `{name}` has incompatible scalar types")]
    IncompatibleAttribute {
        /// Attribute name.
        name: String,
    },
    /// A required affine transform was absent.
    #[error("the spatial reference does not contain an affine transform")]
    MissingAffine,
    /// An affine transform was singular or non-finite.
    #[error("invalid affine transform: {0}")]
    InvalidAffine(String),
    /// A required CRS was absent.
    #[error("the source does not contain a supported horizontal CRS")]
    MissingCrs,
    /// A CRS could not be represented faithfully.
    #[error("unsupported CRS: {0}")]
    UnsupportedCrs(String),
    /// A PROJJSON value was invalid.
    #[error("invalid PROJJSON: {0}")]
    InvalidProjJson(String),
    /// Cubic subdivision reached its fail-closed resource limit.
    #[error("cubic subdivision exceeded maximum depth {max_depth}")]
    SubdivisionLimit {
        /// Maximum permitted recursive depth.
        max_depth: u8,
    },
    /// `GeoTIFF` reference input failed.
    #[error("failed to read GeoTIFF reference `{path}`: {message}")]
    GeoTiff {
        /// Input path.
        path: PathBuf,
        /// Dependency error without exposing its type publicly.
        message: String,
    },
    /// WKB encoding failed.
    #[error("WKB encoding failed: {0}")]
    Wkb(String),
    /// `GeoParquet` construction failed.
    #[error("GeoParquet encoding failed: {0}")]
    GeoParquet(String),
    /// Atomic artifact publication failed.
    #[error("failed to publish `{path}` atomically: {message}")]
    Publication {
        /// Destination path.
        path: PathBuf,
        /// I/O failure description.
        message: String,
    },
}
