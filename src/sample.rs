//! Explicit fixed-interval sampling, independent of adaptive certification.
use crate::{CubicBezier, LineString, Point2, SpatialIoError};

/// Profile identity for uniform Bernstein evaluation at `t = i / intervals`.
pub const UNIFORM_INTERVALS_V1: &str = "uniform_bernstein_intervals_v1";

/// Validated interval count and allocation ceiling for one cubic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UniformSampleOptions {
    intervals: u32,
    vertices: usize,
}
impl UniformSampleOptions {
    /// Requires positive intervals and room for all `intervals + 1` vertices.
    ///
    /// # Errors
    /// Returns a geometry error for zero intervals or a resource error before
    /// allocation when the requested vertex count exceeds the caller ceiling.
    pub fn new(intervals: u32, max_vertices: usize) -> Result<Self, SpatialIoError> {
        if intervals == 0 {
            return Err(SpatialIoError::InvalidGeometry(
                "uniform intervals must be positive".into(),
            ));
        }
        let vertices = usize::try_from(intervals)
            .ok()
            .and_then(|n| n.checked_add(1));
        if vertices.is_none_or(|n| n > max_vertices) {
            return Err(SpatialIoError::ResourceLimit {
                resource: "vertices per cubic",
                limit: max_vertices,
            });
        }
        Ok(Self {
            intervals,
            vertices: vertices.ok_or(SpatialIoError::ResourceLimit {
                resource: "vertices per cubic",
                limit: max_vertices,
            })?,
        })
    }
    /// Number of uniform parameter intervals (the vertex count is one greater).
    #[must_use]
    pub const fn intervals(self) -> u32 {
        self.intervals
    }
}

/// Samples one cubic in source order, retaining duplicate and closed endpoints.
///
/// This profile provides no adaptive deviation guarantee. Its mathematical
/// sampling rule matches the Java Toolset's interval interpretation; arithmetic
/// order and runtime libraries can produce different intermediate vertex bits.
/// Source controls should be normalized into the desired sampling frame first.
///
/// # Errors
/// Returns a typed non-finite error if evaluation overflows. Endpoints are
/// copied exactly, including signed-zero bits; controls are never emitted as
/// vertices merely because they are controls.
pub fn sample_cubic(
    cubic: &CubicBezier,
    options: UniformSampleOptions,
) -> Result<LineString, SpatialIoError> {
    let mut points = Vec::with_capacity(options.vertices);
    points.push(cubic.p0);
    for i in 1..options.intervals {
        let t = f64::from(i) / f64::from(options.intervals);
        let u = 1.0 - t;
        let weights = [
            u.powi(3),
            3.0 * u.powi(2) * t,
            3.0 * u * t.powi(2),
            t.powi(3),
        ];
        let controls = [cubic.p0, cubic.p1, cubic.p2, cubic.p3];
        let coordinate =
            |axis: fn(Point2) -> f64| weights.iter().zip(controls).map(|(w, p)| w * axis(p)).sum();
        points.push(Point2::new(coordinate(Point2::x), coordinate(Point2::y))?);
    }
    points.push(cubic.p3);
    LineString::new(points)
}
