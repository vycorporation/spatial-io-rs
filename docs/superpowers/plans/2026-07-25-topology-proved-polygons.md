# Topology-Proved Polygons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add crate-owned, validated `LinearRing`, `Polygon`, and
`MultiPolygon` values plus deterministic GeoParquet 1.1 WKB output proved by
synthetic fixtures and independent readers.

**Architecture:** A dependency-light `topology` module owns exact finite-f64
validation and explicit shell/hole/multipart roles. `GeometryV1` carries the
validated values, while the optional GeoParquet adapter privately converts
them to `geo-types` for WKB. No caller input is repaired, auto-closed, or
classified by winding.

**Tech Stack:** Rust 1.97.1 / MSRV 1.92, existing crate-owned geometry types,
existing `geo-types`, `wkb`, Arrow 58, Parquet 58, QGIS 4.2, DuckDB Spatial
1.5.3, and SedonaDB 0.4.0.

## Global Constraints

- Keep default features empty and add no system GDAL, PROJ, GEOS, database,
  GUI, Rerun, or vectorizer dependency.
- Require explicit ring closure: at least four positions and exact equality of
  first and last.
- Reject zero-length edges, zero-area rings, self-crossing, self-touching, and
  overlapping ring edges with typed `InvalidGeometry` errors.
- Preserve caller winding exactly. Report numeric-coordinate winding, do not
  infer shell/hole roles from it, and omit GeoParquet `orientation` metadata.
- A `Polygon` has one explicit exterior and zero or more explicit interiors.
  Holes must be strictly inside the shell, boundary-disjoint, pairwise
  disjoint, and non-nested.
- A `MultiPolygon` is non-empty. Component interiors must be disjoint; isolated
  boundary-point contact is accepted, while crossing or shared boundary
  segments are rejected.
- Closed `LineString` and `CubicPath` values remain linework. Polygon
  constructors never auto-promote or repair them.
- Preserve `FeatureV1` identity, attributes, grouping, conversion profile, and
  tolerance unchanged.
- Continue emitting explicit `crs: null` for pixel/local/unknown coordinates.
- Keep existing LineString bytes and behavior unchanged.

---

### Task 1: Public topology model

**Files:**
- Create: `src/topology.rs`
- Modify: `src/model.rs`
- Modify: `src/lib.rs`
- Create: `tests/polygon_model_contract.rs`

**Interfaces:**
- Produces: `RingWinding::{Clockwise, CounterClockwise}`.
- Produces: `LinearRing::new(Vec<Point2>)`, `points()`, and `winding()`.
- Produces: `Polygon::new(LinearRing, Vec<LinearRing>)`, `exterior()`, and
  `interiors()`.
- Produces: `MultiPolygon::new(Vec<Polygon>)` and `polygons()`.
- Extends: `GeometryV1::{Polygon, MultiPolygon}`.

- [x] **Step 1: Write failing literal ring tests**

Add tests that expect a closed square to preserve its exact point order and
winding, and that expect open, too-short, zero-edge, zero-area, bow-tie, and
self-touching rings to return `SpatialIoError::InvalidGeometry`.

- [x] **Step 2: Run the ring tests and verify RED**

Run:

```bash
cargo test --no-default-features --test polygon_model_contract ring -- --nocapture
```

Expected: compile failure because `LinearRing` and `RingWinding` do not exist.

- [x] **Step 3: Implement the minimal validated ring**

Add exact finite-f64 shoelace winding, closed-segment relation, and
non-adjacent edge checks in `src/topology.rs`. Predicate overflow must return an
error instead of guessing.

- [x] **Step 4: Run the ring tests and verify GREEN**

Run the Step 2 command and require all ring tests to pass.

- [x] **Step 5: Write failing polygon and multipart tests**

Use literal rectangles to prove explicit shell/hole roles, winding
independence, hole containment, boundary rejection, nested-hole rejection,
disjoint multiparts, isolated point contact, overlap rejection, and shared-edge
rejection. Also prove a closed invalid `LineString` remains constructible while
`LinearRing::new` rejects the same points.

- [x] **Step 6: Run the polygon tests and verify RED**

Run:

```bash
cargo test --no-default-features --test polygon_model_contract polygon -- --nocapture
cargo test --no-default-features --test polygon_model_contract multipolygon -- --nocapture
```

Expected: compile failure because `Polygon` and `MultiPolygon` do not exist.

- [x] **Step 7: Implement explicit polygon and multipart validation**

Add boundary-disjoint hole validation, point-in-ring classification, pairwise
hole checks, and component-interior overlap checks. Extend and re-export
`GeometryV1` without changing existing variants.

- [x] **Step 8: Run all model tests and verify GREEN**

```bash
cargo test --no-default-features --test polygon_model_contract
cargo test --no-default-features --test model_contract
```

### Task 2: Polygon GeoParquet writer

**Files:**
- Modify: `src/geoparquet.rs`
- Create: `tests/polygon_geoparquet.rs`

**Interfaces:**
- Consumes: validated `GeometryV1::Polygon` and `GeometryV1::MultiPolygon`.
- Produces: GeoParquet WKB with exact metadata types `Polygon` and
  `MultiPolygon`, including mixed polygon/multipolygon collections.

- [x] **Step 1: Write failing writer tests**

Create a collection containing one polygon-with-hole and one multipolygon.
Assert literal GeoParquet metadata types, WKB geometry types and ring counts,
row order, attributes, identity/provenance columns, aggregate bounds, and
byte-identical repeated output.

- [x] **Step 2: Run writer tests and verify RED**

```bash
cargo test --features geoparquet --test polygon_geoparquet -- --nocapture
```

Expected: writer rejection with `UnsupportedPrimitive`.

- [x] **Step 3: Implement dynamic supported-geometry encoding**

Refactor the adapter to collect supported `LineString`, `Polygon`, and
`MultiPolygon` rows, derive unique metadata types in stable
`LineString`/`Polygon`/`MultiPolygon` order, compute bounds from all geometry
points, and privately convert polygon values into `geo-types` for WKB.

- [x] **Step 4: Run writer and existing regression tests GREEN**

```bash
cargo test --features geoparquet --test polygon_geoparquet
cargo test --features geoparquet --test geoparquet_contract
cargo test --features geoparquet --test interoperability_fixtures
```

### Task 3: Synthetic checked-in matrix and independent readers

**Files:**
- Modify: `examples/generate_interoperability_fixtures.rs`
- Modify: `fixtures/interoperability/manifest.json`
- Create: `fixtures/interoperability/polygon.parquet`
- Create: `fixtures/interoperability/multipolygon.parquet`
- Modify: `fixtures/interoperability/README.md`
- Modify: `tests/interoperability_fixtures.rs`
- Modify: `docs/validation/2026-07-25-interoperability-fixture-matrix.md`

**Interfaces:**
- Produces: `spatial_io_interoperability_fixture_matrix_v2`.
- Adds: one shell-with-hole Polygon and one two-component MultiPolygon.

- [x] **Step 1: Extend the fixture contract test and verify RED**

Expect five exact fixture filenames, v2 manifest identity, Polygon and
MultiPolygon metadata/WKB types, literal row counts and extents.

- [x] **Step 2: Run the fixture test and verify RED**

```bash
cargo test --all-features --test interoperability_fixtures
```

Expected: v1 identity and missing polygon files.

- [x] **Step 3: Extend the deterministic generator and regenerate**

```bash
cargo run --example generate_interoperability_fixtures \
  --features geoparquet -- fixtures/interoperability
```

Generate only literal synthetic coordinates and retain the repository's
MIT OR Apache-2.0 fixture license.

- [x] **Step 4: Verify fixture GREEN and reproducibility**

Run the generator twice, compare all five Parquet and manifest SHA-256 values,
then run the fixture contract test.

- [x] **Step 5: Validate independent readers**

Run QGIS `gdal:ogrinfojson`, DuckDB Spatial geometry type/validity/extent
queries, and SedonaDB `read_parquet(validate=True)` against both new files.
Record exact versions, commands, row counts, ring-bearing geometry types,
extents, CRS behavior, and limitations.

### Task 4: Human contract documentation

**Files:**
- Create: `docs/polygon-topology.md`
- Modify: `README.md`
- Modify: `docs/attributes.md`

- [x] **Step 1: Document exact semantics**

Explain closure, simplicity, exact-predicate failure, explicit shell/hole
roles, winding preservation, multipart validity, closed-line fallback,
provenance, GeoParquet orientation omission, and Rerun/vectorizer boundaries.

- [x] **Step 2: Link operator/consumer entry points**

Add concise README and attribute/provenance links without claiming a
vectorizer or filled-Rerun adapter.

### Task 5: Full validation and publication

**Files:**
- Modify: GitHub issue `spatial-io-rs#2`

- [x] **Step 1: Run the complete repository gate**

Run every command from `AGENTS.md`, inspect `cargo tree`, reject forbidden
runtime dependencies, and require a clean diff.

- [x] **Step 2: Update the live acceptance ledger**

Check only criteria supported by implementation, synthetic fixtures,
independent readers, and the full gate. Re-fetch and require zero unchecked
criteria.

- [ ] **Step 3: Commit, push, open PR, re-fetch, and merge**

Commit under `vy-matt-davis`, push
`codex/issue-2-topology-proved-polygons`, create a ready PR closing #2,
re-fetch the issue/PR, merge only with zero unchecked criteria, and
fast-forward the clean main checkout.
