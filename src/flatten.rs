//! Deterministic tolerance-bounded cubic flattening.

use crate::{CubicBezier, CubicPath, LineString, Point2, SpatialIoError};

const MAX_SUBDIVISION_DEPTH: u8 = 32;

/// Options for deterministic cubic subdivision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlattenOptions {
    tolerance: f64,
}

impl FlattenOptions {
    /// Creates options with a finite, positive tolerance in input units.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialIoError::InvalidTolerance`] for zero, negative, or
    /// non-finite input.
    pub fn new(tolerance: f64) -> Result<Self, SpatialIoError> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(SpatialIoError::InvalidTolerance(tolerance));
        }
        Ok(Self { tolerance })
    }

    /// Returns the effective tolerance.
    #[must_use]
    pub const fn tolerance(self) -> f64 {
        self.tolerance
    }
}

/// Derived linework and its conversion provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedLineString {
    /// Resulting portable `LineString`.
    pub line: LineString,
    /// Source primitive identities in path order.
    pub source_primitive_ids: Vec<String>,
    /// Stable conversion-profile identity.
    pub profile_id: &'static str,
    /// Effective tolerance in the input coordinate space.
    pub tolerance: f64,
    /// Number of De Casteljau subdivision operations.
    pub subdivision_count: u64,
}

/// Flattens one cubic into a certified `LineString`.
///
/// # Errors
///
/// Returns a subdivision-limit or geometry error when a certified result
/// cannot be produced.
pub fn flatten_cubic(
    cubic: &CubicBezier,
    options: FlattenOptions,
) -> Result<DerivedLineString, SpatialIoError> {
    let mut points = vec![cubic.p0];
    let mut subdivision_count = 0;
    flatten_recursive(
        cubic,
        options.tolerance,
        0,
        &mut points,
        &mut subdivision_count,
    )?;
    Ok(DerivedLineString {
        line: LineString::new(points)?,
        source_primitive_ids: Vec::new(),
        profile_id: "recursive_convex_hull_bound_v1",
        tolerance: options.tolerance,
        subdivision_count,
    })
}

/// Flattens a connected cubic path without duplicating seam vertices.
///
/// # Errors
///
/// Returns an error when the source identity count differs from the segment
/// count or when certified subdivision cannot complete.
pub fn flatten_cubic_path(
    path: &CubicPath,
    source_primitive_ids: Vec<String>,
    options: FlattenOptions,
) -> Result<DerivedLineString, SpatialIoError> {
    if source_primitive_ids.len() != path.segments().len() {
        return Err(SpatialIoError::InvalidGeometry(
            "source primitive id count must match cubic segment count".to_owned(),
        ));
    }
    let mut points = vec![path.segments()[0].p0];
    let mut subdivision_count = 0;
    for cubic in path.segments() {
        flatten_recursive(
            cubic,
            options.tolerance,
            0,
            &mut points,
            &mut subdivision_count,
        )?;
    }
    Ok(DerivedLineString {
        line: LineString::new(points)?,
        source_primitive_ids,
        profile_id: "recursive_convex_hull_bound_v1",
        tolerance: options.tolerance,
        subdivision_count,
    })
}

fn flatten_recursive(
    cubic: &CubicBezier,
    tolerance: f64,
    depth: u8,
    points: &mut Vec<Point2>,
    subdivision_count: &mut u64,
) -> Result<(), SpatialIoError> {
    if is_flat_enough(cubic, tolerance) {
        if points.last().copied() != Some(cubic.p3) {
            points.push(cubic.p3);
        }
        return Ok(());
    }
    if depth == MAX_SUBDIVISION_DEPTH {
        return Err(SpatialIoError::SubdivisionLimit {
            max_depth: MAX_SUBDIVISION_DEPTH,
        });
    }
    let (left, right) = split_half(cubic)?;
    *subdivision_count += 1;
    flatten_recursive(&left, tolerance, depth + 1, points, subdivision_count)?;
    flatten_recursive(&right, tolerance, depth + 1, points, subdivision_count)
}

fn is_flat_enough(cubic: &CubicBezier, tolerance: f64) -> bool {
    point_segment_distance(cubic.p1, cubic.p0, cubic.p3) <= tolerance
        && point_segment_distance(cubic.p2, cubic.p0, cubic.p3) <= tolerance
}

fn point_segment_distance(point: Point2, start: Point2, end: Point2) -> f64 {
    let dx = end.x() - start.x();
    let dy = end.y() - start.y();
    let length_squared = dx.mul_add(dx, dy * dy);
    if length_squared == 0.0 {
        return (point.x() - start.x()).hypot(point.y() - start.y());
    }
    let projection = (((point.x() - start.x()) * dx + (point.y() - start.y()) * dy)
        / length_squared)
        .clamp(0.0, 1.0);
    let nearest_x = dx.mul_add(projection, start.x());
    let nearest_y = dy.mul_add(projection, start.y());
    (point.x() - nearest_x).hypot(point.y() - nearest_y)
}

#[allow(clippy::similar_names)]
fn split_half(cubic: &CubicBezier) -> Result<(CubicBezier, CubicBezier), SpatialIoError> {
    let p01 = midpoint(cubic.p0, cubic.p1)?;
    let p12 = midpoint(cubic.p1, cubic.p2)?;
    let p23 = midpoint(cubic.p2, cubic.p3)?;
    let p012 = midpoint(p01, p12)?;
    let p123 = midpoint(p12, p23)?;
    let p0123 = midpoint(p012, p123)?;
    Ok((
        CubicBezier::new(cubic.p0, p01, p012, p0123),
        CubicBezier::new(p0123, p123, p23, cubic.p3),
    ))
}

fn midpoint(left: Point2, right: Point2) -> Result<Point2, SpatialIoError> {
    Point2::new((left.x() + right.x()) * 0.5, (left.y() + right.y()) * 0.5)
}
