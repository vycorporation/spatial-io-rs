//! Primitive-neutral spatial conversion and geospatial artifact I/O.
//!
//! The dependency-light core owns geometry, coordinate-reference, and cubic
//! flattening contracts. Optional features add narrowly scoped format adapters.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod flatten;
mod model;
mod numeric;
mod reference;
mod sample;
mod topology;

#[cfg(feature = "geoparquet")]
mod geoparquet;
#[cfg(feature = "geotiff")]
mod geotiff;

pub use error::SpatialIoError;
pub use flatten::{DerivedLineString, FlattenOptions, flatten_cubic, flatten_cubic_path};
pub use model::{
    AttributeFieldV1, AttributeType, AttributeValue, CubicBezier, CubicPath, FeatureCollectionV1,
    FeatureV1, GeometryV1, LineString, Point2,
};
pub use reference::{
    Affine2D, AxisDirection, CoordinateSpace, Crs, PixelAnchor, PixelOrigin, RasterInterpretation,
    SpatialReference, transform_line_string,
};
pub use sample::{UNIFORM_INTERVALS_V1, UniformSampleOptions, sample_cubic};
pub use topology::{LinearRing, MultiPolygon, Polygon, RingWinding};

#[cfg(feature = "geoparquet")]
pub use geoparquet::{
    GeoParquetStreamOptions, GeoParquetStreamReport, GeoParquetWriteOptions, WriteReport,
    write_geoparquet, write_line_strings_geoparquet,
};
#[cfg(feature = "geotiff")]
pub use geotiff::{GeoTiffReference, read_geotiff_reference};
