# Polygon topology contract

`spatial-io` accepts polygon output only through validated, crate-owned
`LinearRing`, `Polygon`, and `MultiPolygon` values. A closed path is not
automatically a polygon, and the library never repairs or reclassifies caller
geometry.

## Linear rings

`LinearRing::new` requires:

- at least four positions;
- exact equality between the first and last position;
- finite coordinates and finite topology calculations;
- no zero-length edge;
- non-zero signed area; and
- a simple boundary: non-adjacent edges cannot cross, touch, or overlap, and
  adjacent edges may meet only at their shared endpoint.

The constructor retains the exact coordinate order. It does not add a closing
position, remove points, snap coordinates, or reverse winding. Predicates use
deterministic finite `f64` arithmetic and fail with
`SpatialIoError::InvalidGeometry` when an intermediate calculation overflows;
they are not an arbitrary-precision repair engine.

`LinearRing::winding` reports the sign of the numeric-coordinate shoelace area:
positive is `CounterClockwise` and negative is `Clockwise`. This is a report,
not a role classifier. In a y-down pixel space, visual clockwise/counterclockwise
appearance is naturally reversed from a conventional y-up map.

Deserialization routes through the same constructors. Invalid serialized
topology and a serialized winding value that disagrees with the coordinates
are rejected rather than creating an unvalidated value.

## Explicit shell and hole roles

`Polygon::new(exterior, interiors)` assigns roles from its arguments:

- `exterior` is the shell regardless of winding;
- each item in `interiors` is a hole regardless of winding;
- every hole must be strictly inside the shell;
- hole boundaries may not touch or cross the shell;
- holes must be pairwise disjoint and cannot touch, overlap, or nest.

Because roles are explicit, callers may use either winding for either role.
The GeoParquet writer preserves that order and intentionally omits the optional
`orientation` metadata field rather than making a winding claim it does not
enforce.

## Multipart grouping

`MultiPolygon::new` requires at least one validated polygon and preserves
caller order. Component interiors must be disjoint. Components may meet at one
or more isolated boundary points, but boundary crossings, shared boundary
segments, overlap, and containment are rejected.

Multipart grouping is also explicit: the constructor never merges nearby
polygons or splits one polygon into components.

## Invalid closed paths remain linework

`LineString` and `CubicPath` retain their existing linework semantics even when
closed. A closed `LineString` that self-intersects can remain linework while
`LinearRing::new` rejects the same positions. There is no auto-promotion,
polygonization, buffering, validity repair, or fallback from a rejected ring.
A producer that wants polygon output must decide shell, holes, and multipart
membership before constructing the topology types.

This keeps source meaning attributable: `spatial-io` validates a producer's
topology decision, but does not invent one.

## GeoParquet and provenance

With the `geoparquet` feature, `write_geoparquet` accepts validated
`LineString`, `Polygon`, and `MultiPolygon` rows and writes GeoParquet 1.1 WKB.
The metadata lists exactly the geometry types present in the collection.
Pixel, local, and unknown coordinate spaces continue to emit explicit
`"crs": null`.

For every row, the writer preserves:

- `feature_id`;
- `source_primitive_id`;
- `group_id`;
- `conversion_profile_id`;
- `conversion_tolerance`; and
- the declared typed attributes.

These fields are how a caller records the original primitive, the explicit
polygon grouping decision, and any conversion method. Polygon support does not
change canonical vectorizer curve artifacts, create a vectorizer adapter, or
define filled-polygon rendering in Rerun.

## Standards boundary and evidence

The contract follows the OGC Simple Features requirement that polygon rings be
closed and simple. GeoParquet 1.1 WKB supports `Polygon` and `MultiPolygon` and
makes its `orientation` declaration optional. The implementation chooses a
stricter, deterministic subset for holes and multipart contact so validity is
unambiguous without a topology runtime dependency.

Synthetic shell, hole, multipart, invalid-ring, and determinism cases are
covered by Rust tests. Checked GeoParquet fixtures are under
[`fixtures/interoperability/`](../fixtures/interoperability/), with QGIS,
DuckDB Spatial, and SedonaDB results recorded in
[`docs/validation/2026-07-25-interoperability-fixture-matrix.md`](validation/2026-07-25-interoperability-fixture-matrix.md).

References:

- [OGC Simple Feature Access, Part 1](https://portal.ogc.org/files/?artifact_id=829)
- [GeoParquet 1.1 specification](https://geoparquet.org/releases/v1.1.0/)
