# AGENTS.md - vycorporation/spatial-io-rs

Read this file before changing geometry, coordinate, CRS, GeoTIFF, GeoParquet,
or publication behavior.

## Repository role

`spatial-io` is the reusable, viewer-independent Rust library for converting
typed primitives into non-geographic or georeferenced spatial artifacts.
Keep the core library independent of `vectorizer-rs`, Rerun, rendering, styling,
database engines, and system geospatial libraries.

## Contract rules

- Keep public types crate-owned.
- Keep default features empty and format dependencies optional.
- Preserve exact six-coefficient affine transforms, pixel anchoring, and raster
  interpretation.
- Never infer WGS84 or another CRS from missing metadata.
- Emit explicit GeoParquet `crs: null` for pixel, local, or unknown coordinates.
- Never treat cubic control points as LineString vertices.
- Preserve deterministic subdivision, ordering, source identity, attributes,
  conversion profile, tolerance, and coordinate provenance.
- Keep closed paths as linework until polygon topology is separately approved
  and proved.
- Reject unsupported metadata or primitives with typed errors.
- Stage writers completely and publish atomically.
- Do not use unsafe Rust.

## Dependency policy

Prefer maintained, pure-Rust libraries with a plausible cross-compilation path.
Isolate all dependency-specific types behind crate-owned interfaces.
Do not add GDAL, C PROJ, GEOS, DuckDB, SedonaDB, Whitebox, Rerun, or GUI
runtime dependencies without a separately approved issue and evidence.

An interoperability tool does not automatically belong in the runtime.
If an adapter drops rotation, skew, CRS, raster anchoring, attributes, or
provenance, reject it or wrap it with explicit validation.

## Consumer boundaries

- `vectorizer-rs` retains canonical cubic artifacts and `preview.png`.
- Rerun retains graph, UI, viewer, and rendering behavior.
- `attribute-styling-rs` retains classification, filtering, ramps, and styling.
- Consumers own their translation into `spatial-io` types.

## Issue and GitHub workflow

Use `codex/` branch names for Codex-authored changes.
All repository and GitHub work for the current project is performed as
`vy-matt-davis`.
Use GitHub issues for implementation work.
Treat acceptance checkboxes as the execution ledger: check only evidence-backed
criteria, re-fetch before PR-ready/merge/close, and never merge or close while
an applicable criterion remains unchecked.

## Validation

Before reporting code complete, run:

```bash
cargo check --no-default-features
cargo check --features geotiff
cargo check --features geoparquet
cargo check --all-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo tree --all-features
git diff --check
```

Confirm the dependency tree contains no GDAL, C PROJ, GEOS, database, GUI, or
Rerun runtime.
Do not claim GeoTIFF or GeoParquet behavior without fixture-backed tests and an
independent reader assertion.
