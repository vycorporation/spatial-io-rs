//! Explicit polygon topology contracts.

use serde::{Deserialize, Serialize};

use crate::{Point2, SpatialIoError};

/// Numeric-coordinate winding of a validated linear ring.
///
/// This reports the sign of the shoelace area in the coordinates as stored.
/// It does not infer whether a ring is a shell or a hole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RingWinding {
    /// Negative signed area.
    Clockwise,
    /// Positive signed area.
    CounterClockwise,
}

/// A simple, explicitly closed, non-zero-area linear ring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LinearRingWire")]
pub struct LinearRing {
    points: Vec<Point2>,
    winding: RingWinding,
}

/// One polygon with an explicit exterior ring and explicit interior holes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PolygonWire")]
pub struct Polygon {
    exterior: LinearRing,
    interiors: Vec<LinearRing>,
}

#[derive(Deserialize)]
struct LinearRingWire {
    points: Vec<Point2>,
    winding: RingWinding,
}

#[derive(Deserialize)]
struct PolygonWire {
    exterior: LinearRing,
    interiors: Vec<LinearRing>,
}

impl Polygon {
    /// Creates a polygon without inferring ring roles from winding.
    ///
    /// Every interior must be strictly inside the exterior. Interior
    /// boundaries may not touch or cross the exterior or each other, and
    /// interiors may not overlap or nest.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialIoError::InvalidGeometry`] when the explicit shell and
    /// holes do not form one valid polygonal surface.
    pub fn new(exterior: LinearRing, interiors: Vec<LinearRing>) -> Result<Self, SpatialIoError> {
        for interior in &interiors {
            if rings_relation(&exterior, interior)? != SegmentRelation::Disjoint
                || point_in_ring(interior.points()[0], &exterior)? != PointLocation::Inside
            {
                return Err(invalid(
                    "a polygon interior must lie strictly inside its exterior",
                ));
            }
        }
        for first in 0..interiors.len() {
            for second in (first + 1)..interiors.len() {
                if rings_relation(&interiors[first], &interiors[second])?
                    != SegmentRelation::Disjoint
                    || point_in_ring(interiors[first].points()[0], &interiors[second])?
                        == PointLocation::Inside
                    || point_in_ring(interiors[second].points()[0], &interiors[first])?
                        == PointLocation::Inside
                {
                    return Err(invalid(
                        "polygon interiors must be pairwise disjoint and non-nested",
                    ));
                }
            }
        }
        Ok(Self {
            exterior,
            interiors,
        })
    }

    /// Returns the explicitly assigned exterior shell.
    #[must_use]
    pub const fn exterior(&self) -> &LinearRing {
        &self.exterior
    }

    /// Returns the explicitly assigned interior holes in caller order.
    #[must_use]
    pub fn interiors(&self) -> &[LinearRing] {
        &self.interiors
    }
}

/// A non-empty collection of polygons with disjoint interiors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "MultiPolygonWire")]
pub struct MultiPolygon {
    polygons: Vec<Polygon>,
}

#[derive(Deserialize)]
struct MultiPolygonWire {
    polygons: Vec<Polygon>,
}

impl MultiPolygon {
    /// Creates a multipart polygon while preserving caller order.
    ///
    /// Component interiors must be disjoint. Isolated boundary-point contact
    /// is accepted, while boundary crossing, shared boundary segments,
    /// overlap, and containment are rejected.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialIoError::InvalidGeometry`] for an empty collection or
    /// components that do not satisfy the multipart contract.
    pub fn new(polygons: Vec<Polygon>) -> Result<Self, SpatialIoError> {
        if polygons.is_empty() {
            return Err(invalid("a multipolygon requires at least one polygon"));
        }
        for first in 0..polygons.len() {
            for second in (first + 1)..polygons.len() {
                validate_disjoint_polygons(&polygons[first], &polygons[second])?;
            }
        }
        Ok(Self { polygons })
    }

    /// Returns component polygons in caller order.
    #[must_use]
    pub fn polygons(&self) -> &[Polygon] {
        &self.polygons
    }
}

impl LinearRing {
    /// Validates and retains an explicitly closed sequence of finite points.
    ///
    /// The first and last points must be exactly equal and at least four
    /// positions must be supplied. Zero-length edges, zero area,
    /// self-crossing, self-touching, overlapping edges, and non-finite
    /// topology predicates are rejected. Coordinates are never reordered,
    /// auto-closed, or repaired.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialIoError::InvalidGeometry`] when the positions do not
    /// form a valid linear ring.
    pub fn new(points: Vec<Point2>) -> Result<Self, SpatialIoError> {
        if points.len() < 4 {
            return Err(invalid("a linear ring requires at least four positions"));
        }
        if points.first() != points.last() {
            return Err(invalid("a linear ring must be explicitly closed"));
        }
        if points.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("a linear ring cannot contain a zero-length edge"));
        }
        validate_simple_boundary(&points)?;
        let area = signed_area_twice(&points)?;
        let winding = if area > 0.0 {
            RingWinding::CounterClockwise
        } else if area < 0.0 {
            RingWinding::Clockwise
        } else {
            return Err(invalid("a linear ring must enclose non-zero area"));
        };
        Ok(Self { points, winding })
    }

    /// Returns the exact, explicitly closed positions in caller order.
    #[must_use]
    pub fn points(&self) -> &[Point2] {
        &self.points
    }

    /// Returns winding in the numeric coordinate space as stored.
    #[must_use]
    pub const fn winding(&self) -> RingWinding {
        self.winding
    }
}

impl TryFrom<LinearRingWire> for LinearRing {
    type Error = SpatialIoError;

    fn try_from(value: LinearRingWire) -> Result<Self, Self::Error> {
        let ring = Self::new(value.points)?;
        if ring.winding != value.winding {
            return Err(invalid(
                "serialized linear ring winding does not match its coordinates",
            ));
        }
        Ok(ring)
    }
}

impl TryFrom<PolygonWire> for Polygon {
    type Error = SpatialIoError;

    fn try_from(value: PolygonWire) -> Result<Self, Self::Error> {
        Self::new(value.exterior, value.interiors)
    }
}

impl TryFrom<MultiPolygonWire> for MultiPolygon {
    type Error = SpatialIoError;

    fn try_from(value: MultiPolygonWire) -> Result<Self, Self::Error> {
        Self::new(value.polygons)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentRelation {
    Disjoint,
    Touch,
    Cross,
    Overlap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointLocation {
    Outside,
    Boundary,
    Inside,
}

fn validate_simple_boundary(points: &[Point2]) -> Result<(), SpatialIoError> {
    let edge_count = points.len() - 1;
    for first in 0..edge_count {
        for second in (first + 1)..edge_count {
            let adjacent = second == first + 1 || (first == 0 && second + 1 == edge_count);
            let relation = segment_relation(
                points[first],
                points[first + 1],
                points[second],
                points[second + 1],
            )?;
            if (adjacent && relation != SegmentRelation::Touch)
                || (!adjacent && relation != SegmentRelation::Disjoint)
            {
                return Err(invalid(
                    "a linear ring boundary must be simple and may meet only at adjacent endpoints",
                ));
            }
        }
    }
    Ok(())
}

fn signed_area_twice(points: &[Point2]) -> Result<f64, SpatialIoError> {
    let mut area = 0.0;
    for edge in points.windows(2) {
        let term = edge[0]
            .x()
            .mul_add(edge[1].y(), -(edge[1].x() * edge[0].y()));
        area += term;
        if !term.is_finite() || !area.is_finite() {
            return Err(invalid("linear ring area evaluation overflowed"));
        }
    }
    Ok(area)
}

fn validate_disjoint_polygons(first: &Polygon, second: &Polygon) -> Result<(), SpatialIoError> {
    match polygons_boundary_relation(first, second)? {
        SegmentRelation::Cross | SegmentRelation::Overlap => {
            return Err(invalid(
                "multipolygon components may not cross or share boundary segments",
            ));
        }
        SegmentRelation::Disjoint | SegmentRelation::Touch => {}
    }
    if exterior_has_point_inside(first, second)? || exterior_has_point_inside(second, first)? {
        return Err(invalid("multipolygon component interiors must be disjoint"));
    }
    Ok(())
}

fn exterior_has_point_inside(
    candidate: &Polygon,
    container: &Polygon,
) -> Result<bool, SpatialIoError> {
    candidate
        .exterior()
        .points()
        .iter()
        .take(candidate.exterior().points().len() - 1)
        .try_fold(false, |inside, &point| {
            Ok(inside || point_in_polygon(point, container)? == PointLocation::Inside)
        })
}

fn point_in_polygon(point: Point2, polygon: &Polygon) -> Result<PointLocation, SpatialIoError> {
    match point_in_ring(point, polygon.exterior())? {
        PointLocation::Outside => Ok(PointLocation::Outside),
        PointLocation::Boundary => Ok(PointLocation::Boundary),
        PointLocation::Inside => {
            for interior in polygon.interiors() {
                match point_in_ring(point, interior)? {
                    PointLocation::Inside => return Ok(PointLocation::Outside),
                    PointLocation::Boundary => return Ok(PointLocation::Boundary),
                    PointLocation::Outside => {}
                }
            }
            Ok(PointLocation::Inside)
        }
    }
}

fn point_in_ring(point: Point2, ring: &LinearRing) -> Result<PointLocation, SpatialIoError> {
    let mut inside = false;
    for edge in ring.points().windows(2) {
        if orientation(edge[0], edge[1], point)? == 0.0 && on_segment(edge[0], edge[1], point) {
            return Ok(PointLocation::Boundary);
        }
        let crosses_y = (edge[0].y() > point.y()) != (edge[1].y() > point.y());
        if crosses_y {
            let x_at_y = (edge[1].x() - edge[0].x()).mul_add(
                (point.y() - edge[0].y()) / (edge[1].y() - edge[0].y()),
                edge[0].x(),
            );
            if !x_at_y.is_finite() {
                return Err(invalid("point-in-ring evaluation overflowed"));
            }
            if point.x() < x_at_y {
                inside = !inside;
            }
        }
    }
    Ok(if inside {
        PointLocation::Inside
    } else {
        PointLocation::Outside
    })
}

fn polygons_boundary_relation(
    first: &Polygon,
    second: &Polygon,
) -> Result<SegmentRelation, SpatialIoError> {
    let mut aggregate = SegmentRelation::Disjoint;
    for first_ring in std::iter::once(first.exterior()).chain(first.interiors()) {
        for second_ring in std::iter::once(second.exterior()).chain(second.interiors()) {
            aggregate = stronger(aggregate, rings_relation(first_ring, second_ring)?);
        }
    }
    Ok(aggregate)
}

fn rings_relation(
    first: &LinearRing,
    second: &LinearRing,
) -> Result<SegmentRelation, SpatialIoError> {
    let mut aggregate = SegmentRelation::Disjoint;
    for first_edge in first.points().windows(2) {
        for second_edge in second.points().windows(2) {
            aggregate = stronger(
                aggregate,
                segment_relation(first_edge[0], first_edge[1], second_edge[0], second_edge[1])?,
            );
        }
    }
    Ok(aggregate)
}

const fn stronger(first: SegmentRelation, second: SegmentRelation) -> SegmentRelation {
    use SegmentRelation::{Cross, Disjoint, Overlap, Touch};
    match (first, second) {
        (Overlap, _) | (_, Overlap) => Overlap,
        (Cross, _) | (_, Cross) => Cross,
        (Touch, _) | (_, Touch) => Touch,
        (Disjoint, Disjoint) => Disjoint,
    }
}

fn segment_relation(
    a: Point2,
    b: Point2,
    c: Point2,
    d: Point2,
) -> Result<SegmentRelation, SpatialIoError> {
    let ab_c = orientation(a, b, c)?;
    let ab_d = orientation(a, b, d)?;
    let cd_a = orientation(c, d, a)?;
    let cd_b = orientation(c, d, b)?;

    if ab_c == 0.0 && ab_d == 0.0 && cd_a == 0.0 && cd_b == 0.0 {
        return Ok(collinear_relation(a, b, c, d));
    }
    if opposite(ab_c, ab_d) && opposite(cd_a, cd_b) {
        return Ok(SegmentRelation::Cross);
    }
    if (ab_c == 0.0 && on_segment(a, b, c))
        || (ab_d == 0.0 && on_segment(a, b, d))
        || (cd_a == 0.0 && on_segment(c, d, a))
        || (cd_b == 0.0 && on_segment(c, d, b))
    {
        return Ok(SegmentRelation::Touch);
    }
    Ok(SegmentRelation::Disjoint)
}

fn orientation(a: Point2, b: Point2, c: Point2) -> Result<f64, SpatialIoError> {
    let edge_x = b.x() - a.x();
    let edge_y = b.y() - a.y();
    let point_x = c.x() - a.x();
    let point_y = c.y() - a.y();
    let value = edge_x.mul_add(point_y, -(edge_y * point_x));
    if [edge_x, edge_y, point_x, point_y, value]
        .into_iter()
        .all(f64::is_finite)
    {
        Ok(value)
    } else {
        Err(invalid("topology orientation evaluation overflowed"))
    }
}

fn opposite(first: f64, second: f64) -> bool {
    (first > 0.0 && second < 0.0) || (first < 0.0 && second > 0.0)
}

fn on_segment(a: Point2, b: Point2, point: Point2) -> bool {
    point.x() >= a.x().min(b.x())
        && point.x() <= a.x().max(b.x())
        && point.y() >= a.y().min(b.y())
        && point.y() <= a.y().max(b.y())
}

#[allow(clippy::float_cmp)]
fn collinear_relation(a: Point2, b: Point2, c: Point2, d: Point2) -> SegmentRelation {
    let use_x = (b.x() - a.x()).abs() >= (b.y() - a.y()).abs();
    let (a0, a1, b0, b1) = if use_x {
        (a.x(), b.x(), c.x(), d.x())
    } else {
        (a.y(), b.y(), c.y(), d.y())
    };
    let overlap_start = a0.min(a1).max(b0.min(b1));
    let overlap_end = a0.max(a1).min(b0.max(b1));
    if overlap_start < overlap_end {
        SegmentRelation::Overlap
    } else if overlap_start == overlap_end {
        SegmentRelation::Touch
    } else {
        SegmentRelation::Disjoint
    }
}

fn invalid(message: &str) -> SpatialIoError {
    SpatialIoError::InvalidGeometry(message.to_owned())
}
