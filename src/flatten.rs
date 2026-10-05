//! Deterministic tolerance-bounded cubic flattening.

use crate::{CubicBezier, CubicPath, LineString, Point2, SpatialIoError};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};

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
    let (exact, certification) = exact_cubic(cubic, options);
    flatten_recursive(
        &exact,
        &certification,
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
    for cubic in path.segments() {
        let (exact, certification) = exact_cubic(cubic, options);
        flatten_recursive(
            &exact,
            &certification,
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
    cubic: &[ScaledPoint; 4],
    certification: &Certification,
    depth: u8,
    points: &mut Vec<Point2>,
    subdivision_count: &mut u64,
) -> Result<(), SpatialIoError> {
    let (_, rounded_start) = rounded_endpoint(&cubic[0], certification)?;
    let (end, rounded_end) = rounded_endpoint(&cubic[3], certification)?;
    // Distance to a segment is convex: certifying all four exact control
    // points against the rounded output chord certifies the entire subcurve.
    if cubic.iter().all(|point| {
        within_segment_distance(
            point,
            &rounded_start,
            &rounded_end,
            &certification.tolerance_squared,
        )
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
    flatten_recursive(&left, certification, depth + 1, points, subdivision_count)?;
    flatten_recursive(&right, certification, depth + 1, points, subdivision_count)
}

#[derive(Clone)]
struct ScaledPoint {
    x: BigInt,
    y: BigInt,
}

struct Certification {
    scale: usize,
    denominator: BigInt,
    floating_scale: f64,
    tolerance_squared: BigInt,
    tolerance: f64,
}

fn exact_cubic(cubic: &CubicBezier, options: FlattenOptions) -> ([ScaledPoint; 4], Certification) {
    let coordinates = [cubic.p0, cubic.p1, cubic.p2, cubic.p3]
        .map(|point| [rational(point.x()), rational(point.y())]);
    let tolerance = rational(options.tolerance);
    // Every finite binary64 is dyadic. Reserve three denominator bits per
    // De Casteljau level, so all half-sums through depth 32 are exact integer
    // divisions. A shared scale cancels from every distance comparison and
    // avoids repeated rational normalization in the recursion.
    let scale = coordinates
        .iter()
        .flatten()
        .chain(std::iter::once(&tolerance))
        .map(denominator_exponent)
        .max()
        .expect("four control points")
        + 3 * usize::from(MAX_SUBDIVISION_DEPTH);
    let tolerance = scaled_value(&tolerance, scale);
    let certification = Certification {
        scale,
        denominator: BigInt::one() << scale,
        floating_scale: 2.0_f64.powi(-i32::try_from(scale).expect("bounded binary64 scale")),
        tolerance_squared: &tolerance * &tolerance,
        tolerance: options.tolerance,
    };
    let points = coordinates.map(|[x, y]| ScaledPoint {
        x: scaled_value(&x, scale),
        y: scaled_value(&y, scale),
    });
    (points, certification)
}

fn rational(value: f64) -> BigRational {
    BigRational::from_float(value).expect("validated finite coordinate or tolerance")
}

fn denominator_exponent(value: &BigRational) -> usize {
    usize::try_from(value.denom().bits() - 1).expect("binary64 denominator has at most 1075 bits")
}

fn scaled_value(value: &BigRational, scale: usize) -> BigInt {
    value.numer() << (scale - denominator_exponent(value))
}

fn squared_distance(first: &ScaledPoint, second: &ScaledPoint) -> BigInt {
    let x = &first.x - &second.x;
    let y = &first.y - &second.y;
    &x * &x + &y * &y
}

fn rounded_endpoint(
    point: &ScaledPoint,
    certification: &Certification,
) -> Result<(Point2, ScaledPoint), SpatialIoError> {
    let error = || SpatialIoError::ApproximationPrecision {
        tolerance: certification.tolerance,
    };
    // ToPrimitive computes the ratio without requiring a reduced fraction.
    // Keep rational conversion outside the integer distance/subdivision work.
    let round = |coordinate: &BigInt| {
        let scaled = coordinate
            .to_f64()
            .map(|value| value * certification.floating_scale);
        if let Some(value) = scaled.filter(|value| value.is_normal() || coordinate.is_zero()) {
            return Some(value);
        }
        BigRational::new_raw(coordinate.clone(), certification.denominator.clone()).to_f64()
    };
    let rounded = Point2::new(
        round(&point.x).ok_or_else(error)?,
        round(&point.y).ok_or_else(error)?,
    )?;
    let scale = |coordinate: f64| {
        let bits = coordinate.to_bits();
        let biased_exponent = i32::try_from((bits >> 52) & 0x7ff).expect("11 exponent bits");
        let fraction = bits & ((1_u64 << 52) - 1);
        let (significand, exponent) = if biased_exponent == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1_u64 << 52), biased_exponent - 1075)
        };
        if significand == 0 {
            return Ok(BigInt::zero());
        }
        // Remove trailing zero bits so rounding to a binary64 value on this
        // exact dyadic grid cannot require a finer denominator than the grid.
        let zeros = significand.trailing_zeros();
        let exponent = exponent + i32::try_from(zeros).expect("at most 53 significand bits");
        let shift = certification
            .scale
            .checked_add_signed(isize::try_from(exponent).expect("bounded binary64 exponent"))
            .ok_or_else(error)?;
        let value = BigInt::from(significand >> zeros) << shift;
        Ok(if coordinate.is_sign_negative() {
            -value
        } else {
            value
        })
    };
    let exact_rounded = ScaledPoint {
        x: scale(rounded.x())?,
        y: scale(rounded.y())?,
    };
    if squared_distance(point, &exact_rounded) > certification.tolerance_squared {
        return Err(error());
    }
    Ok((rounded, exact_rounded))
}

fn within_segment_distance(
    point: &ScaledPoint,
    start: &ScaledPoint,
    end: &ScaledPoint,
    tolerance_squared: &BigInt,
) -> bool {
    let dx = &end.x - &start.x;
    let dy = &end.y - &start.y;
    let wx = &point.x - &start.x;
    let wy = &point.y - &start.y;
    let length_squared = &dx * &dx + &dy * &dy;
    let projection = &wx * &dx + &wy * &dy;
    if length_squared.is_zero() || projection <= BigInt::zero() {
        return squared_distance(point, start) <= *tolerance_squared;
    }
    if projection >= length_squared {
        return squared_distance(point, end) <= *tolerance_squared;
    }
    let cross = &wx * &dy - &wy * &dx;
    &cross * &cross <= tolerance_squared * length_squared
}

#[allow(clippy::similar_names)]
fn split_half(cubic: &[ScaledPoint; 4]) -> ([ScaledPoint; 4], [ScaledPoint; 4]) {
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

fn midpoint(left: &ScaledPoint, right: &ScaledPoint) -> ScaledPoint {
    ScaledPoint {
        x: (&left.x + &right.x) >> 1,
        y: (&left.y + &right.y) >> 1,
    }
}
