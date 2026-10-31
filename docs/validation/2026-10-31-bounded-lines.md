# Bounded uniform LineString conversion

Base: cbfd2c077e806f9030726c3e41de0870bceeca91. Validation host: Apple Silicon,
Rust 1.97.1. Debug execution is test-only; no runtime-speed or total-RSS claim.

The uniform profile uses N intervals and N+1 vertices, preserving exact endpoints.
It is separate from unchanged adaptive deviation certification and does not
establish actual Java-runtime bit parity. The minimal consumer runs without
format features.

The iterator writer preserves declared schemas, typed attributes and row order.
It supports zero-row output, nullable values and late source/cancellation errors.
No-clobber publication follows complete sync and attestation. Fixed 512-row
Parquet groups and per-row encoding mini-batches preserve bytes across caller
chunk sizes, including dictionary/page threshold crossings.

Admission bounds: 256 attribute fields/64 KiB names, 1 MiB accounted encoded feature
payload, 64 MiB accounted conversion batch, 16 profiles and 16,777,216 rows. Schema
slots count even for absent nullable values. These are not total-RSS ceilings:
Arrow/WKB copies, format buffers and retained footer metadata remain additional.
The caller owns input construction/storage and chooses its processing ceilings.

Executed successfully with at most 2 compile jobs:

- cargo check --offline --no-default-features
- cargo check --offline --features geotiff
- cargo check --offline --features geoparquet
- cargo check --offline --all-features
- cargo fmt --check
- cargo clippy --offline --all-targets --all-features -- -D warnings
- cargo test --offline --all-features: 61 passed, zero failures/ignored
- RUSTDOCFLAGS="-D warnings" cargo doc --offline --no-deps --all-features
- cargo tree --offline --all-features: no GDAL, C PROJ, GEOS, database, GUI or Rerun runtime
- cargo run --offline --no-default-features --example uniform_line
- cargo +1.92.0 check --offline --no-default-features
- git diff --check

All-optional-feature MSRV qualification remains pending.

Independent source review identified row-group ordinal exhaustion,
chunk-dependent dictionary/page thresholds and Arrow32 offset overflow.
Focused regressions cover those fixes, decoded empty schemas/footer, schema
limits, endpoint bits and late cancellation. Follow-up source review found no
publication blocker and requested explicit bounded-schema/RSS wording.

Producer integration, external runtime interoperability and performance are
separate evidence gates. No proprietary engine adapter or dataset is included.
