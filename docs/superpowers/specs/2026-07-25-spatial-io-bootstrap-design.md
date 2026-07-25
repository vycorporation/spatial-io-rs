# spatial-io Bootstrap Design

**Status:** Proposed for written review  
**Date:** 2026-07-25  
**Repository:** `vycorporation/spatial-io`  
**Visibility:** Public

## Purpose

`spatial-io` will be the reusable Rust library for converting typed spatial
primitives into standards-valid spatial artifacts. Its coordinate model will
support pixel space, local or engineering space, and georeferenced world space
without treating all spatial data as geographic.

The first product need is write-oriented: export current cubic Bezier geometry
from `vycorporation/vectorizer-rs` and future geometry from the
`vycorporation/rerun` fork. The repository name deliberately does not include
`export` because validated readers and additional format adapters may be added
later under separate issues. The bootstrap will not implement speculative
readers.

## Repository and Package Identity

- GitHub repository: `vycorporation/spatial-io`
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
GDAL, PROJ, or GEOS types.

## Coordinate Model

Every feature collection declares exactly one coordinate space:

- **Pixel** — image or canvas coordinates with explicit origin, axis direction,
  and pixel anchoring where relevant;
- **Local** — unit-bearing Cartesian coordinates without an assigned CRS; or
- **Georeferenced** — world coordinates with an explicit CRS representation.

Conversions may include an explicit six-coefficient affine transform.
Coordinate conversion is never inferred from dimensions, filenames, or
attribute names.

For GeoParquet:

- georeferenced coordinates carry valid PROJJSON;
- unknown or local coordinates use an explicit `crs: null`;
- the writer must not omit the `crs` field for pixel or local coordinates,
  because omission means OGC:CRS84 in GeoParquet 1.1; and
- x/y output order follows the GeoParquet/WKB contract regardless of the
  declared CRS axis order.

## Primitive-Neutral Geometry Model

The first public geometry model will be closed and versioned rather than
pretending every producer emits LineStrings. It will reserve explicit variants
for:

- point and multipoint;
- line string and multilinestring;
- polygon and multipolygon with holes;
- cubic Bezier path segments; and
- future approved analytic or vector-native primitives.

New primitive variants require explicit conversion and compatibility review.
The model will be non-exhaustive to downstream consumers where Rust
compatibility requires it.

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

## Format Roadmap

The first durable writer is:

- GeoParquet 1.1 using portable WKB geometry.

The next likely writer is:

- GeoJSON for inspectability and independent reference fixtures.

FlatGeobuf and other spatial formats require separate evidence-backed issues.
Reading GeoParquet, GeoJSON, or raster formats is not part of the first
implementation. The broader name permits future readers only after a concrete
consumer and contract exist.

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

- raster decoding and raster compute;
- contour tracing and cubic fitting;
- canonical cubic geometry;
- geometry audits;
- `curves.parquet`;
- `preview.png`; and
- per-image and batch artifact contracts.

Its future adapter will translate audited `CubicBezier` records and coordinate
metadata into `spatial-io` source features. `spatial-io` will derive portable
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

- route primitive-neutral spatial conversion to `vycorporation/spatial-io`;
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

The implementation plan will require:

- exact fixtures for pixel, local, and georeferenced coordinates;
- direct primitive round-trip tests where the format supports the primitive;
- bounded-approximation tests for cubic and later analytic curves;
- topology fixtures for shells, holes, multipart geometry, invalid rings, and
  closed non-polygonal paths;
- GeoParquet 1.1 metadata-schema validation;
- independent interoperability checks in DuckDB Spatial, QGIS, and an
  Arrow/GeoArrow-family reader for supported output;
- deterministic output and provenance tests;
- resource-limit and atomic-publication tests; and
- consumer adapter contract tests in `vectorizer-rs` and Rerun.

The repository gate is:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
git diff --check
```

## Explicit Non-Goals for the Bootstrap

- raster, GeoTIFF, COG, or image decoding;
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
3. Implement GeoParquet 1.1 WKB linework export with cubic approximation.
4. Add topology-proved polygonal output.
5. Add the Rerun and vectorizer adapters through separate issues.
6. Add later readers or writers only when a concrete consumer requires them.
