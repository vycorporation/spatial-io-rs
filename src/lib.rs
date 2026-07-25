//! Primitive-neutral spatial conversion and geospatial artifact I/O.
//!
//! The dependency-light core owns geometry, coordinate-reference, and cubic
//! flattening contracts. Optional features add narrowly scoped format adapters.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod flatten;
mod model;
mod reference;

#[cfg(feature = "geoparquet")]
mod geoparquet;
#[cfg(feature = "geotiff")]
mod geotiff;

pub use error::SpatialIoError;
pub use flatten::{DerivedLineString, FlattenOptions, flatten_cubic, flatten_cubic_path};
pub use model::{
    AttributeValue, CubicBezier, CubicPath, FeatureCollectionV1, FeatureV1, GeometryV1, LineString,
    Point2,
};
pub use reference::{
    Affine2D, AxisDirection, CoordinateSpace, Crs, PixelAnchor, PixelOrigin, RasterInterpretation,
    SpatialReference, transform_line_string,
};

#[cfg(feature = "geoparquet")]
pub use geoparquet::{GeoParquetWriteOptions, WriteReport, write_geoparquet};
#[cfg(feature = "geotiff")]
pub use geotiff::{GeoTiffReference, read_geotiff_reference};
