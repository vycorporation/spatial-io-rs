# spatial-io Bootstrap Design

**Status:** Approved with GeoTIFF-reference revision
**Date:** 2026-07-25
**Repository:** `vycorporation/spatial-io-rs`
**Visibility:** Public

## Purpose

`spatial-io` will be the reusable Rust library for converting typed spatial
primitives into standards-valid spatial artifacts. Its coordinate model will
support pixel space, local or engineering space, and georeferenced world space
without treating all spatial data as geographic.

The first product need is to convert current cubic Bezier geometry from
`vycorporation/vectorizer-rs` and future geometry from the
`vycorporation/rerun` fork into non-geographic or georeferenced linework.
Georeferencing may be supplied explicitly or read from the GeoTIFF that
produced the curves. The repository name deliberately does not include
`export` because validated readers and additional format adapters may be added
later without renaming the crate.

## Repository and Package Identity

- GitHub repository: `vycorporation/spatial-io-rs`
- Cargo package: `spatial-io`
- Rust crate path: `spatial_io`
- License: dual MIT or Apache-2.0
- Rust edition: 2024
- Minimum supported Rust version: 1.89, matching the lower requirement of the
  initial consumers

The crate is public so the public Vy Rerun fork can depend on it without
requiring private repository credentials.

## Architectural Seam

The library interface accepts:

1. stable feature identities;
2. a versioned, typed source primitive;
3. optional typed attributes;
4. an explicit coordinate-reference contract; and
5. a validated conversion and output profile.

It returns either:

- a validated, portable feature collection suitable for one or more writers;
- a completed spatial artifact and deterministic report; or
- a typed error before publishing a misleading or partial artifact.

Consumers own adapters into this interface. The public interface will not
expose Arrow, Parquet, DataFusion, Rerun, vectorizer, image, wgpu, Whitebox,
GeoTIFF-reader, GDAL, PROJ, or GEOS types.

## Coordinate Model

Every feature collection declares exactly one coordinate space:

- **Pixel** — image or canvas coordinates with explicit origin, axis direction,
  and pixel anchoring where relevant;
- **Local** — unit-bearing Cartesian coordinates without an assigned CRS; or
- **Georeferenced** — world coordinates with an explicit CRS representation.

Conversions may include an explicit six-coefficient affine transform.
Coordinate conversion is never inferred from dimensions, filenames, or
attribute names.

Pixel coordinates declare whether integer coordinates refer to pixel corners
or centers. A GeoTIFF adapter preserves PixelIsArea or PixelIsPoint and returns
a corner-normalized affine plus the original raster interpretation. A caller
must still declare how its generated primitive coordinates are anchored; the
library does not assume that curve coordinates inherit the source raster's
sample anchoring.

For GeoParquet:

- georeferenced coordinates carry valid PROJJSON;
- unknown or local coordinates use an explicit `crs: null`;
- the writer must not omit the `crs` field for pixel or local coordinates,
  because omission means OGC:CRS84 in GeoParquet 1.1; and
- x/y output order follows the GeoParquet/WKB contract regardless of the
  declared CRS axis order.

## Primitive-Neutral Geometry Model

The first public geometry model will be closed and versioned rather than
pretending every producer emits LineStrings. It defines explicit variants for:

- point;
- line string and multilinestring;
- cubic Bezier path segments; and
- future approved analytic or vector-native primitives.

New primitive variants require explicit conversion and compatibility review.
Multipoint, polygon, and multipolygon variants are deferred until a concrete
consumer contract and, for polygonal types, topology evidence exist. The model
will be non-exhaustive to downstream consumers where Rust compatibility
requires it.

Each writer or conversion profile declares whether a primitive:

- maps directly to the destination representation;
- requires a deterministic, tolerance-bounded approximation;
- requires grouping or topology construction;
- is retained as native provenance beside a derived geometry; or
- is rejected as unsupported.

Portable output must not erase source primitive meaning. A derived feature
records source feature and primitive identity, conversion profile identity,
effective tolerance, coordinate transformation, and grouping provenance.

## Curve and Topology Conversion

GeoParquet/WKB does not provide a portable nonlinear cubic geometry type.
Cubic and future analytic curves therefore require deterministic approximation
when exported to WKB linework or polygonal rings.

The conversion contract will define:

- the coordinate space in which error is bounded;
- maximum approximation deviation;
- stable subdivision and endpoint rules;
- ordering and grouping;
- preservation of exact source identities;
- open-path and closed-path behavior; and
- failure when numerical or resource limits are exceeded.

A closed path is not automatically a polygon. Polygon conversion requires
proved ring closure, shell/hole classification, winding policy, validity
checks, and multipart grouping. Until that contract is implemented and tested,
closed curves remain linework rather than being mislabeled as valid polygons.

## Feature and Attribute Model

Features carry:

- stable feature identity;
- source primitive identity;
- geometry;
- typed scalar attributes;
- optional grouping and layer identity; and
- conversion provenance.

The initial scalar attribute model supports null, Boolean, signed and unsigned
integers, finite floating-point values, binary, and UTF-8 strings. Writers
reject unsupported values rather than stringifying them silently.

`attribute-styling-rs` remains an independent module. It resolves visual style
from attributes; `spatial-io` preserves attributes and geometry in spatial
artifacts. Adapters may connect the two without either crate owning the other
crate's product behavior.

## GeoTIFF Reference Input

The first read path is deliberately narrow: read the spatial reference needed
to map pixel or native-image coordinates into model coordinates. It does not
make raster decoding part of the core geometry API.

The optional `geotiff` feature will use the pure-Rust
`geotiff-reader`/`geotiff-core` 0.7 line. This implementation was selected over
the current Whitebox high-level adapter because it preserves both
`ModelPixelScaleTag` plus `ModelTiepointTag` and
`ModelTransformationTag`, and it normalizes PixelIsArea/PixelIsPoint
semantics without discarding rotation or skew. Whitebox Next Gen remains a
credible future broader-raster adapter and an interoperability reference, but
the first implementation must not wrap a path known to flatten full affine
metadata.

The GeoTIFF reference reader returns crate-owned values containing:

- image dimensions and band count;
- the exact six-coefficient corner-normalized affine;
- original PixelIsArea or PixelIsPoint interpretation;
- EPSG authority identity when recognized;
- complete caller-supplied PROJJSON when required for custom CRS output;
- nodata text when present; and
- a source-format and dependency-version provenance record.

Recognized EPSG codes are resolved to standards-valid PROJJSON through an
isolated, exact-version dependency. A user-defined or unsupported CRS fails
with a typed error unless the caller supplies matching PROJJSON explicitly.
The adapter never relabels unknown coordinates as WGS84.

GeoTIFF, BigTIFF, tiled COG layout, rotated/skewed affine transforms,
PixelIsArea, and PixelIsPoint are in the fixture profile. Remote HTTP range
reading, raster warping, automatic band interpretation, alpha/mask fusion, and
reprojection are deferred.

## Dependency Policy

The first implementation pins current pure-Rust libraries behind crate-owned
interfaces:

- `geo-types` 0.7 for established geometry interoperability in private
  adapters and tests;
- `wkb` 0.9 for OGC WKB encoding and independent decoding;
- Apache Arrow/Parquet 58 plus `geoparquet` 0.8 for GeoParquet 1.1 writing and
  validation;
- `geotiff-reader` and `geotiff-core` 0.7 for exact local GeoTIFF spatial
  metadata; and
- an EPSG-to-PROJJSON resolver pinned to an exact version and isolated behind
  the crate-owned CRS contract.

DuckDB Spatial, SedonaDB, QGIS, and GeoArrow are consumers used for
interoperability validation, not runtime dependencies of the core library.
Database engines are not embedded merely to serialize geometry.

Dependencies must be maintained, documented, compatible with Rust 1.89, and
free of required system GDAL, PROJ, or GEOS installations. Any dependency that
silently drops CRS, affine, raster anchoring, or attribute information is
rejected or wrapped with typed validation.

## Format Roadmap

The first durable writer is:

- GeoParquet 1.1 using portable WKB geometry.

The next likely writer is:

- GeoJSON for inspectability and independent reference fixtures.

FlatGeobuf and other spatial formats require separate evidence-backed issues.
Reading GeoParquet or GeoJSON is not part of the first implementation. The
only initial reader is the optional, metadata-focused GeoTIFF reference
adapter described above.

The GeoParquet writer will:

- emit required `geo` file metadata;
- declare the exact geometry types present;
- use valid WKB in a root binary geometry column;
- record correct CRS, orientation, edges, and bounding-box metadata when
  asserted;
- use `.parquet` for maximum interoperability;
- preserve typed attributes and provenance; and
- validate metadata against the official GeoParquet 1.1 schema.

## vectorizer-rs Relationship

`vectorizer-rs` remains authoritative for:

- its current raster decoding and raster compute;
- contour tracing and cubic fitting;
- canonical cubic geometry;
- geometry audits;
- `curves.parquet`;
- `preview.png`; and
- per-image and batch artifact contracts.

Its future adapter will translate audited `CubicBezier` records and coordinate
metadata into `spatial-io` source features. For GeoTIFF inputs it may ask
`spatial-io` to read the source spatial reference while retaining the
vectorizer's exact raster-compute contract. `spatial-io` will derive portable
linework or later validated polygonal output without changing the canonical
cubic rows.

No current `preview.png`, manifest v5, Parquet metadata v2, or
`vectorizer_per_image_artifact_contract_v5` behavior changes during the
`spatial-io` bootstrap.

Vectorizer issue `#74` currently mixes reusable spatial conversion with raster
input and vectorizer artifact concerns. After this specification is approved
and the replacement issue exists:

- close `vectorizer-rs#74` as not planned rather than attempting to delete
  GitHub history;
- comment with the replacement `spatial-io` issue and the narrowed ownership
  decision; and
- create a future vectorizer integration issue only when the shared writer
  contract is ready to consume.

## Rerun Relationship

Rerun may adapt points, lines, polygons, cubic Beziers, or future graph-owned
geometry into the same source-feature model and invoke the same writers.
Rerun is not required to read the produced artifacts.

Rerun remains responsible for:

- graph and chunk storage;
- UI commands and project intent;
- viewer selection and interaction;
- GPU rendering; and
- mapping Rerun-native primitive identities into the shared interface.

`spatial-io` must not depend on Rerun crates or encode Rerun UI behavior.

## Repository Documentation

The bootstrap will include:

- `README.md` describing current write-first scope and consumer relationships;
- `AGENTS.md` as the canonical repository guidance;
- `CLAUDE.md` as a symlink to `AGENTS.md`;
- `CONTEXT.md` with the domain glossary and ownership rules;
- dual `LICENSE-MIT` and `LICENSE-APACHE`;
- a library-first Cargo scaffold;
- local verification scripts or a `Justfile` where useful; and
- issue templates suitable for contract and implementation work.

`vectorizer-rs` and Rerun documentation will link to this repository at their
respective adapter seams without claiming an integration that has not landed.

## Skills Repository

The spatial conversion rules are cross-repository, standards-sensitive, and
easy to implement incorrectly, so a source-linked `spatial-io` skill is
justified once the repository contract is committed.

The skill will:

- route primitive-neutral spatial conversion to `vycorporation/spatial-io-rs`;
- read current format, coordinate, topology, and validation contracts from a
  selected source commit;
- preserve vectorizer and Rerun ownership;
- reject false CRS assumptions and invalid polygon promotion; and
- avoid copying versioned schemas or commands from the product repository.

The existing `vectorizer-rs` skill will be updated to route derived spatial
export through `spatial-io` and to stop treating issue `#74` as an
implementation target. Installer metadata, README indexes, OpenAI metadata,
and trigger tests will be updated together under the skills repository's
validator.

## Errors, Publication, and Determinism

Typed errors distinguish:

- unsupported source primitive;
- invalid or non-finite coordinates;
- invalid coordinate-reference metadata;
- incompatible affine transformation;
- approximation failure or resource limit;
- invalid ring or topology;
- unsupported attribute type;
- destination format limitation;
- metadata-schema failure; and
- artifact publication failure.

Writers stage complete outputs and publish atomically where the filesystem
allows it. No partially written artifact is reported as successful.

For identical ordered input features and a validated profile, geometry,
attributes, metadata, and report identities are deterministic.

## Testing and Validation

The bootstrap implementation plan requires:

- exact fixtures for pixel, local, and georeferenced coordinates;
- direct primitive round-trip tests where the format supports the primitive;
- bounded-approximation tests for cubic and later analytic curves;
- rejection of unsupported polygon promotion and preservation of closed paths
  as linework;
- GeoParquet 1.1 metadata-schema validation;
- independent programmatic validation through the Parquet, GeoParquet, and WKB
  libraries;
- deterministic output and provenance tests;
- resource-limit and atomic-publication tests; and
- documentation of the future consumer adapter boundaries in `vectorizer-rs`
  and Rerun.

Manual QGIS validation, DuckDB Spatial and SedonaDB interoperability, topology
fixtures, and consumer adapter contract tests belong to follow-up issues. They
must not be checked speculatively as bootstrap evidence.

The repository gate is:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
git diff --check
```

## Explicit Non-Goals for the Bootstrap

- making general-purpose raster pixels part of the core geometry model;
- remote COG access or raster warping;
- automatic imagery band, alpha, or mask interpretation;
- GeoParquet or GeoJSON reading;
- rendering or visual styling;
- replacing vectorizer-native cubic artifacts;
- pretending cubic control points are LineString vertices;
- polygon output before topology is proved;
- GUI or Rerun runtime dependencies;
- system GDAL, PROJ, or GEOS requirements;
- exposing Arrow or Parquet types in the public interface; or
- implementing multiple speculative formats in the first slice.

## Delivery Sequence

1. Bootstrap and validate the public `spatial-io` library repository.
2. Implement the primitive, coordinate, attribute, and conversion contracts.
3. Implement deterministic cubic-to-LineString approximation.
4. Implement the optional GeoTIFF spatial-reference adapter.
5. Implement GeoParquet 1.1 WKB linework export.
6. Add topology-proved polygonal output under a later issue.
7. Add the Rerun and vectorizer adapters through separate issues.
8. Add later readers or writers only when a concrete consumer requires them.
