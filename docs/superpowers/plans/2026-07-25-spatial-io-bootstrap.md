# spatial-io Bootstrap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a reusable Rust library that converts cubic Bezier paths into non-geographic or GeoTIFF-georeferenced LineStrings and writes standards-valid GeoParquet 1.1 WKB artifacts.

**Architecture:** Keep crate-owned geometry, coordinate, attribute, CRS, and provenance types in the public API. Implement deterministic cubic subdivision and affine transformation in the dependency-light core, then place GeoTIFF reference reading and GeoParquet writing behind optional features. Integrate the repository through documentation and source-linked skills without changing `vectorizer-rs` artifacts or Rerun runtime behavior.

**Tech Stack:** Rust 2024/MSRV 1.89, `geo-types` 0.7.19, `wkb` 0.9.2, Arrow/Parquet 58, `geoparquet` 0.8.0, `geotiff-reader`/`geotiff-core` 0.7.0, `epsg-utils` 0.0.3, Serde, thiserror, tempfile, SHA-256.

## Global Constraints

- Repository is public `vycorporation/spatial-io-rs`; Cargo package is `spatial-io` and crate path is `spatial_io`.
- All commits and GitHub actions use `vy-matt-davis <124176547+vy-matt-davis@users.noreply.github.com>`.
- Public APIs expose no Arrow, Parquet, GeoArrow, GeoTIFF-reader, Whitebox, vectorizer, Rerun, GDAL, PROJ, or GEOS types.
- Default features keep GeoTIFF and GeoParquet dependencies optional.
- No system GDAL, PROJ, or GEOS installation is required.
- Cubic control points are never emitted as LineString vertices without deterministic flattening.
- GeoParquet local or unknown coordinate spaces emit `"crs": null`; missing CRS must never accidentally mean OGC:CRS84.
- Existing `vectorizer-rs` `curves.parquet` and `preview.png` behavior remains unchanged.
- Polygon output is rejected/deferred until shell, hole, winding, multipart, and validity semantics are proved.
- Every production behavior follows a failing-test-first red/green cycle.

---

### Task 1: Repository Contract and Library Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `src/lib.rs`
- Create: `src/error.rs`
- Create: `README.md`
- Create: `CONTEXT.md`
- Create: `AGENTS.md`
- Create: `CLAUDE.md` as a symlink to `AGENTS.md`
- Create: `LICENSE-MIT`
- Create: `LICENSE-APACHE`
- Create: `.github/ISSUE_TEMPLATE/bug_report.yml`
- Create: `.github/ISSUE_TEMPLATE/contract_change.yml`
- Create: `.github/ISSUE_TEMPLATE/config.yml`
- Modify: `docs/superpowers/specs/2026-07-25-spatial-io-bootstrap-design.md`

**Interfaces:**
- Produces the `spatial_io` library root and `SpatialIoError` error type used by later tasks.
- Declares features `geotiff` and `geoparquet`, with `default = []`.
- Pins exact 0.x geospatial dependencies and Arrow/Parquet major 58.

- [ ] **Step 1: Add the library manifest and empty module declarations**

Use `rust-version = "1.89"`, `edition = "2024"`, dual licensing, repository metadata, and feature-gated optional dependencies. Keep `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` in `src/lib.rs`.

- [ ] **Step 2: Run the first compilation**

Run: `cargo check --all-features`
Expected: FAIL because the declared modules and public error exports do not exist.

- [ ] **Step 3: Add the minimal error module and public exports**

Define `SpatialIoError` with typed variants for invalid coordinates, invalid tolerance, unsupported primitive, incompatible attributes, missing affine, missing or unsupported CRS, GeoTIFF input, WKB encoding, GeoParquet encoding, and atomic publication.

- [ ] **Step 4: Add repository documentation and governance files**

Document the write-first library role, optional GeoTIFF reference reader, dependency rationale, explicit non-goals, consumer boundaries, validation commands, and security-sensitive file handling. Make `AGENTS.md` canonical and create `CLAUDE.md -> AGENTS.md`.

- [ ] **Step 5: Verify the scaffold**

Run:

```bash
cargo fmt --check
cargo check --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 6: Commit**

```bash
git add .
git commit -m "chore: scaffold spatial io library"
```

### Task 2: Owned Geometry, Attribute, Coordinate, and CRS Contracts

**Files:**
- Create: `src/model.rs`
- Create: `src/reference.rs`
- Test: `tests/model_contract.rs`
- Modify: `src/lib.rs`
- Modify: `src/error.rs`

**Interfaces:**
- Produces `Point2`, `CubicBezier`, `CubicPath`, `LineString`, `GeometryV1`, `FeatureV1`, `AttributeValue`, `CoordinateSpace`, `PixelAnchor`, `RasterInterpretation`, `Affine2D`, `Crs`, and `SpatialReference`.
- `Point2::new(x, y) -> Result<Point2, SpatialIoError>` rejects non-finite values.
- `Affine2D::transform(point, anchor) -> Result<Point2, SpatialIoError>` applies the exact six-coefficient transform after an explicit center offset.

- [ ] **Step 1: Write failing model validation tests**

Test these literal behaviors:

```rust
assert!(Point2::new(f64::NAN, 0.0).is_err());
assert!(Crs::projjson("[]").is_err());
assert_eq!(
    Affine2D::new(100.0, 2.0, 0.5, 200.0, -0.25, -3.0)?
        .transform(Point2::new(4.0, 5.0)?, PixelAnchor::Corner)?,
    Point2::new(110.5, 184.0)?
);
```

Also verify `PixelAnchor::Center` adds exactly `(0.5, 0.5)` before the affine and that a singular affine is rejected.

- [ ] **Step 2: Run the model tests to verify RED**

Run: `cargo test --test model_contract`
Expected: FAIL because the public model types do not exist.

- [ ] **Step 3: Implement minimal owned model types**

Use `BTreeMap<String, AttributeValue>` for deterministic attribute ordering. Support null, bool, i64, u64, finite f64, bytes, and UTF-8 strings. Define initial geometry variants for Point, LineString, MultiLineString, and CubicPath; do not add polygon variants until their topology contract exists.

- [ ] **Step 4: Implement explicit coordinate and CRS validation**

`Crs` has `Epsg(u32)`, `ProjJson(String)`, and `Unknown`. Parse PROJJSON with `serde_json` and require a JSON object. `CoordinateSpace` distinguishes pixel, local, and georeferenced values and never invents a CRS.

- [ ] **Step 5: Run model tests to verify GREEN**

Run: `cargo test --test model_contract`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src tests/model_contract.rs
git commit -m "feat: define spatial geometry contracts"
```

### Task 3: Deterministic Cubic-to-LineString Conversion

**Files:**
- Create: `src/flatten.rs`
- Test: `tests/flatten_contract.rs`
- Modify: `src/lib.rs`
- Modify: `src/error.rs`

**Interfaces:**
- Produces `FlattenOptions::new(tolerance)`, `flatten_cubic`, and `flatten_cubic_path`.
- Produces `DerivedLineString` with `line`, `source_primitive_ids`, `profile_id = "recursive_convex_hull_bound_v1"`, `tolerance`, and `subdivision_count`.
- Guarantees directed curve-to-polyline deviation no greater than the requested tolerance in the input coordinate space.

- [ ] **Step 1: Write failing straight-line and curved-cubic tests**

Use hand-derived fixtures:

```rust
let straight = CubicBezier::new(p(0, 0), p(1, 0), p(2, 0), p(3, 0))?;
assert_eq!(flatten_cubic(&straight, FlattenOptions::new(0.01)?)?.points(), &[p(0, 0), p(3, 0)]);
```

For a quarter-like cubic, independently sample 4,097 parameter values and assert the minimum distance to the emitted polyline is at most the requested tolerance plus `1e-12`.

- [ ] **Step 2: Run flatten tests to verify RED**

Run: `cargo test --test flatten_contract`
Expected: FAIL because flattening does not exist.

- [ ] **Step 3: Implement recursive De Casteljau subdivision**

Accept a segment when both interior control points are within tolerance of the chord segment. This is a convex-hull upper bound on directed curve-to-chord distance. Split at `t = 0.5`, process left before right, deduplicate seams exactly, and fail with a typed resource-limit error at depth 32 rather than returning an uncertified approximation.

- [ ] **Step 4: Add path grouping and validation tests**

Verify connected cubics form one LineString without duplicated seam vertices, disconnected cubics are rejected, zero/negative/non-finite tolerance is rejected, and output is byte-for-byte deterministic across repeated conversions.

- [ ] **Step 5: Run flatten tests to verify GREEN**

Run: `cargo test --test flatten_contract`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src tests/flatten_contract.rs
git commit -m "feat: flatten cubic paths to certified linework"
```

### Task 4: GeoTIFF Spatial-Reference Adapter

**Files:**
- Create: `src/geotiff.rs`
- Test: `tests/geotiff_reference.rs`
- Modify: `src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `src/error.rs`

**Interfaces:**
- Produces feature-gated `read_geotiff_reference(path) -> Result<GeoTiffReference, SpatialIoError>`.
- `GeoTiffReference` contains crate-owned width, height, band count, nodata text, exact `Affine2D`, `RasterInterpretation`, `Crs`, and dependency provenance.
- Recognized EPSG inputs resolve to `Crs::Epsg`; user-defined/unknown CRS returns a typed error rather than WGS84.

- [ ] **Step 1: Write failing in-memory fixture tests**

Generate temporary GeoTIFF fixtures with `geotiff-writer` 0.7.0 for:

- north-up PixelIsArea EPSG:32618;
- PixelIsPoint with the expected half-pixel-normalized corner transform;
- a 4x4 ModelTransformation matrix with rotation/skew;
- BigTIFF layout; and
- a tiled COG-compatible layout.

Assert the six affine values, raster interpretation, dimensions, bands, nodata, and EPSG using literal expected values.

- [ ] **Step 2: Run GeoTIFF tests to verify RED**

Run: `cargo test --all-features --test geotiff_reference`
Expected: FAIL because the adapter does not exist.

- [ ] **Step 3: Implement the feature-gated adapter**

Use `geotiff_reader::GeoTiffFile` and copy its transform and CRS data into crate-owned types. Match all raster-type variants explicitly. Reject missing transforms, unknown raster interpretation, absent CRS, compound/vertical-only CRS, and user-defined CRS that cannot be represented faithfully.

- [ ] **Step 4: Run GeoTIFF tests to verify GREEN**

Run: `cargo test --all-features --test geotiff_reference`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src tests/geotiff_reference.rs
git commit -m "feat: read exact geotiff spatial references"
```

### Task 5: GeoParquet 1.1 WKB Writer

**Files:**
- Create: `src/geoparquet.rs`
- Test: `tests/geoparquet_contract.rs`
- Modify: `src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `src/error.rs`

**Interfaces:**
- Produces feature-gated `write_geoparquet(path, collection, options) -> Result<WriteReport, SpatialIoError>`.
- Writes one ordered row per `FeatureV1` with `feature_id`, `source_primitive_id`, typed attributes, conversion provenance, bbox covering columns, and root binary `geometry`.
- Uses little-endian OGC WKB LineString geometry and GeoParquet 1.1 `geo` metadata.

- [ ] **Step 1: Write failing non-geographic GeoParquet test**

Write two literal LineStrings and assert through Parquet and `geoparquet` readers that:

- geometry physical type is binary;
- WKB decodes as LineString with exact coordinates;
- `geo.version == "1.1.0"`;
- `primary_column == "geometry"`;
- `geometry_types == ["LineString"]`;
- `encoding == "WKB"`;
- `crs` is present and JSON null; and
- bbox and provenance values match the input.

- [ ] **Step 2: Run the GeoParquet test to verify RED**

Run: `cargo test --all-features --test geoparquet_contract`
Expected: FAIL because the writer does not exist.

- [ ] **Step 3: Implement deterministic WKB, Arrow batches, and metadata**

Convert crate-owned linework privately to `geo_types::LineString`, encode with `wkb` 0.9.2, construct Arrow 58 arrays, and construct `geoparquet` 0.8 metadata structs. Use `Some(Value::Null)` for pixel/local/unknown CRS. Resolve `Crs::Epsg` to PROJJSON through the exact `epsg-utils` dependency and validate caller-provided PROJJSON objects.

- [ ] **Step 4: Implement typed attribute columns**

Union attribute names in sorted order. Require one scalar type per name across the collection, represent absent/null values as Arrow nulls, and reject cross-feature type conflicts. Preserve unsigned integers without lossy casts.

- [ ] **Step 5: Implement atomic publication and report**

Write to a named temporary file in the destination directory, flush and sync it, persist it atomically, sync the parent directory on Unix, then compute SHA-256 and return feature count, byte length, checksum, bbox, CRS identity, and conversion profile identities.

- [ ] **Step 6: Add georeferenced, conflict, and publication tests**

Verify EPSG:32618 emits a PROJJSON object with the correct EPSG identifier, invalid PROJJSON fails before publication, attribute type conflicts fail before publication, and an existing unrelated destination is never partially overwritten.

- [ ] **Step 7: Run GeoParquet tests to verify GREEN**

Run: `cargo test --all-features --test geoparquet_contract`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock src tests/geoparquet_contract.rs
git commit -m "feat: write geoparquet linestring artifacts"
```

### Task 6: Public Examples, Dependency Audit, and Repository Validation

**Files:**
- Create: `examples/cubic_to_geoparquet.rs`
- Create: `docs/dependencies.md`
- Modify: `README.md`
- Modify: `CONTEXT.md`
- Modify: `AGENTS.md`
- Test: documentation tests in `src/lib.rs`

**Interfaces:**
- Produces a compile-checked example for local and GeoTIFF-georeferenced linework.
- Documents why database engines are validators rather than core dependencies.

- [ ] **Step 1: Add compile-failing documentation examples first**

Add public API examples that construct a cubic path, flatten it, optionally load a GeoTIFF reference, transform linework, and write GeoParquet.

- [ ] **Step 2: Run documentation tests to verify RED**

Run: `cargo test --doc --all-features`
Expected: FAIL until the examples match the final API exactly.

- [ ] **Step 3: Complete examples and dependency rationale**

Record exact versions, upstream URLs, MSRVs, maintenance status, public/private API boundary, known limitations, and upgrade policy for every geospatial dependency.

- [ ] **Step 4: Validate all feature combinations**

Run:

```bash
cargo check --no-default-features
cargo check --features geotiff
cargo check --features geoparquet
cargo check --all-features
cargo test --all-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo tree --all-features
git diff --check
```

Expected: all commands exit 0 and `cargo tree` contains no GDAL, C PROJ, or GEOS binding.

- [ ] **Step 5: Commit**

```bash
git add README.md CONTEXT.md AGENTS.md docs examples src
git commit -m "docs: document spatial io workflows"
```

### Task 7: Vectorizer and Rerun Ownership References

**Files:**
- Modify in vectorizer worktree: `README.md`
- Modify in vectorizer worktree: `CONTEXT.md`
- Modify in vectorizer worktree: `AGENTS.md`
- Modify in Rerun worktree: `CONTEXT-MAP.md`
- Modify in Rerun worktree: `product/graph-workbench/product-prd/source/README.md`

**Interfaces:**
- Produces documentation-only links to `vycorporation/spatial-io-rs`.
- Creates separate documentation-boundary and adapter issues without adding
  runtime dependencies.

- [ ] **Step 1: Create documentation-boundary and future adapter issues**

Create one bounded documentation issue and one future adapter issue in
`vectorizer-rs`, then do the same in Rerun. Link all four to
`spatial-io-rs#1`; keep existing artifacts and UI behavior unchanged. The
documentation pull requests close only the bounded documentation issues.

- [ ] **Step 2: Add source-of-truth ownership references**

State that `spatial-io-rs` owns reusable curve flattening, affine/CRS representation, GeoTIFF reference reading, and spatial writers. State that vectorizer retains raster compute/canonical curves and Rerun retains graph/UI/rendering.

- [ ] **Step 3: Validate repository prose**

Run:

```bash
git -C <vectorizer-worktree> diff --check
pixi run -C <rerun-worktree> lint-rerun <each-modified-rerun-file>
git -C <rerun-worktree> diff --check
```

Expected: all commands exit 0.

- [ ] **Step 4: Commit each repository**

Use `docs: reference spatial io boundary` in both repositories.

### Task 8: Source-Linked Skills

**Files:**
- Create in skills worktree: `skills/aries-ai/spatial-io/SKILL.md`
- Create in skills worktree: `skills/aries-ai/spatial-io/agents/openai.yaml`
- Modify in skills worktree: `skills/aries-ai/vectorizer-rs/SKILL.md`
- Modify in skills worktree: `skills/aries-ai/vectorizer-rs/agents/openai.yaml` if prompt changes
- Modify in skills worktree: `skills/aries-ai/README.md`
- Modify in skills worktree: `README.md`
- Modify in skills worktree: `.claude-plugin/marketplace.json`
- Modify in skills worktree: `tests/triggers.md`

**Interfaces:**
- Produces a concise source-linked `spatial-io` skill.
- Routes spatial export away from the vectorizer skill while retaining standalone vectorizer execution guidance.

- [ ] **Step 1: Record the baseline trigger failure**

Because subagent dispatch is disabled for this task, add representative trigger/non-trigger cases to the repository’s existing trigger ledger and run the validator before the skill exists. Expected: validator fails on the deliberately referenced missing skill.

- [ ] **Step 2: Initialize and write the source-linked skill**

Use the repository’s established skill layout. Tell agents to read `README.md`, `CONTEXT.md`, and `AGENTS.md` from a selected `spatial-io-rs` commit; keep versioned API names and dependency versions out of the skill body.

- [ ] **Step 3: Update vectorizer routing**

Route requests for derived LineStrings, GeoTIFF-affine application, CRS preservation, and GeoParquet output to `spatial-io`. Keep vectorizer runs, timing, curve counts, previews, and canonical curve artifacts in the vectorizer skill.

- [ ] **Step 4: Validate skill metadata and behavior surfaces**

Run: `uv run scripts/validate_repo.py`
Expected: PASS including marketplace, README, OpenAI metadata, trigger, and script-smoke checks.

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "docs(skills): add spatial io routing"
```

### Task 9: Issue Migration, Pull Requests, Merge, and Final Verification

**Files:**
- Modify: GitHub issue bodies and comments only.

**Interfaces:**
- Publishes all four repositories under `vy-matt-davis`.
- Closes `vectorizer-rs#74` as not planned with replacement links.

- [ ] **Step 1: Update `spatial-io-rs#1` acceptance evidence**

Narrow the issue to the approved bootstrap and move genuinely future
QGIS/SedonaDB/manual integration and adapter work to linked follow-up issues.
Check only bootstrap criteria backed by commits and fresh validation. Add links
to the dependency decision, API docs, fixture tests, validation summary, and
follow-up issues.

- [ ] **Step 2: Push branches and create ready pull requests**

Push each `codex/` branch and open a PR linked to its issue. Verify PR author is `vy-matt-davis`, head SHA matches the local branch, and checks are terminal.

- [ ] **Step 3: Re-run final repository gates immediately before merge**

Re-run Task 6’s full Rust gate, Task 7’s prose gates, Task 8’s skills validator, and `git diff --check` in every worktree.

- [ ] **Step 4: Merge all ready PRs**

Merge only after re-fetching live issue checkboxes and confirming no applicable criterion is unchecked. Use repository-supported squash or merge policy and verify the resulting default-branch commit author.

- [ ] **Step 5: Close `vectorizer-rs#74` as not planned**

Post a comment linking `spatial-io-rs#1` and the two consumer adapter issues. Explain that reusable raster-reference and geometry export moved to `spatial-io-rs`, while vectorizer integration remains separately tracked. Close with reason `not planned`.

- [ ] **Step 6: Verify remote publication and clean worktrees**

Fetch all origins, confirm each merged commit is contained in `origin/main`, confirm GitHub objects were authored by `vy-matt-davis`, and confirm every worktree is clean.
