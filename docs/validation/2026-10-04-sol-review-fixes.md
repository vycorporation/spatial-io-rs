# Sol 6.1 High review corrections — 2026-10-04

## Scope

Version 0.1.1 corrects the five reproduced findings from the review of
`f9fd833`, tracked by [issue #19](https://github.com/vycorporation/spatial-io-rs/issues/19).
See [the changelog](../../CHANGELOG.md) for changed behavior and identifiers.
The review attribution is **Sol 6.1 High**; it does not certify that every
possible defect has been excluded.

## Required validation

Executed directly on Beast. Every command exited 0:

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
git diff --check f9fd833..HEAD
CARGO_TARGET_DIR=/tmp/spatial-io-review-msrv cargo +1.92.0 check --locked --offline --all-features
cargo package --list --allow-dirty --offline
```

All 51 integration tests passed. New cases exercise translated closed cubics
that cannot meet a sub-ULP tolerance, finite coordinates that previously
overflowed midpoint addition, signed subnormal excursions, extreme mixed
coordinate exponents, rejected GeoTIFF model/authority combinations,
complete valid and invalid caller PROJJSON, full spatial-reference metadata,
invalid affine provenance before publication, and 128 concurrent overwrites
whose reports must each attest their own bytes.

The dependency tree contains no GDAL, C PROJ, GEOS, database, GUI, or Rerun
runtime. The optional schema validator has default features disabled and no
HTTP/file resolver dependencies. The package file list includes both schema
snapshots and their licenses. No package was published.

An independent review found no remaining correctness issue in the five fixes.
Its performance finding was resolved by exact dyadic arithmetic. The reviewer
independently reran all 10 flatten tests and compared 96 mixed, extreme, and
subnormal exponent cases against the prior rational certificate; all outputs
and errors matched. The final full validation gate passed after this change.

## Deterministic artifacts and independent readers

The committed fixtures were regenerated with:

```bash
cargo run --example generate_interoperability_fixtures \
  --features geoparquet -- fixtures/interoperability
cargo run --example generate_interoperability_fixtures \
  --features geoparquet -- /tmp/spatial-io-issue-19-regenerated
```

All five Parquet files and the manifest were compared byte for byte and were
identical. The fixture contract test also asserts each committed digest and
length and decodes geometry, attributes, CRS, and full spatial provenance.

GDAL/OGR 3.13.1 independently read all five files:

```bash
mkdir -p /tmp/spatial-io-issue-19-independent
for input in fixtures/interoperability/*.parquet; do
  ogrinfo -so -al -json "$input" > "/tmp/spatial-io-issue-19-independent/$(basename "$input" .parquet).json"
done
```

Assertions on each resulting JSON confirmed its Parquet driver, declared
geometry type, feature count, extent, and nine non-geometry fields against
the manifest. Pixel/local files reported no CRS; the projected file reported
EPSG:32618. PyArrow 23.0.1 independently decoded both `geo` and `spatial_io`
footer metadata in all five files. Assertions confirmed the spatial-reference
schema tag and writer version 0.1.1. Python `jsonschema` Draft 7 validation,
with the bundled PROJJSON schema supplied through a local resolver store,
accepted every `geo` object and the projected CRS's `crs` definition.

The two committed GeoTIFF CRS regressions were also read with `gdalinfo -json`;
see [fixture provenance and assertions](../../fixtures/reference-regressions/README.md).
GDAL distinguishes the custom projected CRS and the absent projected identity
without promoting either geographic base to the raster's horizontal CRS.
Both fixture affines match their six literal coefficients.

These tools are external validators and do not enter the Rust runtime.
The prior QGIS, DuckDB, and SedonaDB observations apply to the 0.1.0 bytes;
they were not rerun for this release.

## Limits

Exact subdivision certification costs more arithmetic and retains a maximum
depth of 32. Unrepresentable endpoints fail with a typed precision error.
Schema validation checks structure, not authority-database consistency or
reprojection suitability. External GIS readers do not automatically interpret
the producer-specific `spatial_io` metadata. A report attests its own published
artifact; a later writer may replace the same destination path.

## Subdivision performance

The independent review identified a substantial cost in the initial rational
implementation: 100 unit quarter-circle cubics at tolerance 0.001, with 33
vertices each, took 740–776 ms in an optimized build, versus 242–247 µs for
the original uncertified floating subdivision. The final implementation uses
exact dyadic integer arithmetic instead of repeated rational normalization.
Subsequent optimized measurements took 16–33 ms for 100 curves, including
an independent repeat at 31 ms. The paired original implementation took
234–460 µs as host load varied. The final correction is approximately
0.16–0.33 ms per such cubic, substantially faster than the initial correction;
exact certification still costs more than the original floating calculation.

A reproducible optimized benchmark, with no timing assertion or extra
benchmark dependency, is now part of the repository:

```bash
cargo bench --bench flatten_batch --no-default-features --locked --offline
```

These measurements describe this curve, machine, and tolerance. Coordinate
exponent ranges, tighter tolerances, and output size change the cost; this is
not a universal throughput claim.
