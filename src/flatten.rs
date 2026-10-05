//! Deterministic tolerance-bounded cubic flattening.

use crate::numeric::ExactPoint;
use crate::{CubicBezier, CubicPath, LineString, Point2, SpatialIoError};
use num_rational::BigRational;
use num_traits::ToPrimitive;

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
/// Returns a subdivision-limit, endpoint-precision, or geometry error when
/// a certified result cannot be produced.
pub fn flatten_cubic(
    cubic: &CubicBezier,
    options: FlattenOptions,
) -> Result<DerivedLineString, SpatialIoError> {
    let mut points = vec![cubic.p0];
    let mut subdivision_count = 0;
    let tolerance_squared = squared_tolerance(options);
    flatten_recursive(
        &exact_cubic(cubic),
        options.tolerance,
        &tolerance_squared,
        0,
        &mut points,
        &mut subdivision_count,
    )?;
    preserve_collapsed_endpoint(&mut points, cubic.p3);
    Ok(DerivedLineString {
        line: LineString::new(points)?,
        source_primitive_ids: Vec::new(),
        profile_id: "recursive_convex_hull_bound_v2",
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
    let tolerance_squared = squared_tolerance(options);
    for cubic in path.segments() {
        flatten_recursive(
            &exact_cubic(cubic),
            options.tolerance,
            &tolerance_squared,
            0,
            &mut points,
            &mut subdivision_count,
        )?;
    }
    if let Some(cubic) = path.segments().last() {
        preserve_collapsed_endpoint(&mut points, cubic.p3);
    }
    Ok(DerivedLineString {
        line: LineString::new(points)?,
        source_primitive_ids,
        profile_id: "recursive_convex_hull_bound_v2",
        tolerance: options.tolerance,
        subdivision_count,
    })
}

fn preserve_collapsed_endpoint(points: &mut Vec<Point2>, endpoint: Point2) {
    if points.len() == 1 {
        points.push(endpoint);
    }
}

fn flatten_recursive(
    cubic: &[ExactPoint; 4],
    tolerance: f64,
    tolerance_squared: &BigRational,
    depth: u8,
    points: &mut Vec<Point2>,
    subdivision_count: &mut u64,
) -> Result<(), SpatialIoError> {
    let start = rounded_endpoint(&cubic[0], tolerance, tolerance_squared)?;
    let end = rounded_endpoint(&cubic[3], tolerance, tolerance_squared)?;
    let rounded_start = ExactPoint::from(start);
    let rounded_end = ExactPoint::from(end);
    // Distance to a segment is convex: certifying all four exact control
    // points against the rounded output chord certifies the entire subcurve.
    if cubic.iter().all(|point| {
        within_segment_distance(point, &rounded_start, &rounded_end, tolerance_squared)
    }) {
        if points.last().copied() != Some(end) {
            points.push(end);
        }
        return Ok(());
    }
    if depth == MAX_SUBDIVISION_DEPTH {
        return Err(SpatialIoError::SubdivisionLimit {
            max_depth: MAX_SUBDIVISION_DEPTH,
        });
    }
    let (left, right) = split_half(cubic);
    *subdivision_count += 1;
    flatten_recursive(
        &left,
        tolerance,
        tolerance_squared,
        depth + 1,
        points,
        subdivision_count,
    )?;
    flatten_recursive(
        &right,
        tolerance,
        tolerance_squared,
        depth + 1,
        points,
        subdivision_count,
    )
}

fn exact_cubic(cubic: &CubicBezier) -> [ExactPoint; 4] {
    [cubic.p0, cubic.p1, cubic.p2, cubic.p3].map(ExactPoint::from)
}

fn squared_tolerance(options: FlattenOptions) -> BigRational {
    let tolerance = BigRational::from_float(options.tolerance).expect("validated finite tolerance");
    &tolerance * &tolerance
}

fn squared_distance(first: &ExactPoint, second: &ExactPoint) -> BigRational {
    let x = &first.x - &second.x;
    let y = &first.y - &second.y;
    &x * &x + &y * &y
}

fn rounded_endpoint(
    point: &ExactPoint,
    tolerance: f64,
    tolerance_squared: &BigRational,
) -> Result<Point2, SpatialIoError> {
    let error = || SpatialIoError::ApproximationPrecision { tolerance };
    let rounded = Point2::new(
        point.x.to_f64().ok_or_else(error)?,
        point.y.to_f64().ok_or_else(error)?,
    )?;
    if squared_distance(point, &rounded.into()) > *tolerance_squared {
        return Err(error());
    }
    Ok(rounded)
}

fn within_segment_distance(
    point: &ExactPoint,
    start: &ExactPoint,
    end: &ExactPoint,
    tolerance_squared: &BigRational,
) -> bool {
    let dx = &end.x - &start.x;
    let dy = &end.y - &start.y;
    let wx = &point.x - &start.x;
    let wy = &point.y - &start.y;
    let length_squared = &dx * &dx + &dy * &dy;
    let projection = &wx * &dx + &wy * &dy;
    if length_squared == BigRational::default() || projection <= BigRational::default() {
        return squared_distance(point, start) <= *tolerance_squared;
    }
    if projection >= length_squared {
        return squared_distance(point, end) <= *tolerance_squared;
    }
    let cross = &wx * &dy - &wy * &dx;
    &cross * &cross <= tolerance_squared * length_squared
}

#[allow(clippy::similar_names)]
fn split_half(cubic: &[ExactPoint; 4]) -> ([ExactPoint; 4], [ExactPoint; 4]) {
    let p01 = midpoint(&cubic[0], &cubic[1]);
    let p12 = midpoint(&cubic[1], &cubic[2]);
    let p23 = midpoint(&cubic[2], &cubic[3]);
    let p012 = midpoint(&p01, &p12);
    let p123 = midpoint(&p12, &p23);
    let p0123 = midpoint(&p012, &p123);
    (
        [cubic[0].clone(), p01, p012, p0123.clone()],
        [p0123, p123, p23, cubic[3].clone()],
    )
}

fn midpoint(left: &ExactPoint, right: &ExactPoint) -> ExactPoint {
    let two = BigRational::from_integer(2.into());
    ExactPoint {
        x: (&left.x + &right.x) / &two,
        y: (&left.y + &right.y) / two,
    }
}
