# Consumer JSON parsing boundary correction

Issue: [#21](https://github.com/vycorporation/spatial-io-rs/issues/21), discovered by
[vectorizer-rs #255](https://github.com/vycorporation/vectorizer-rs/issues/255).
Reviewed with Sol 6.1 High on 2026-10-04 (America/New_York).
Baseline: `3f5c979aa21325a5a0a826c87ec287dda10bb03a`.

The initial 0.1.1 schema validator enabled `serde_json/float_roundtrip` through
Cargo feature unification. That changed unrelated consumer JSON parsing:
420 numeric fields in the committed vectorizer-rs PCB reference decoded
differently, and its pinned strict-v1 report hash changed from
`04c9f61af72e1e905761a4204c1a7e6850f4013532ee3b7cf102320fdfe4cb4f` to
`e6fdf754af610cb58099f857f02a8393b262541788c2bcbdce8909990959b4f0`.
Historical fingerprint expectations were not updated.

The correction exact-pins jsonschema 0.47.0, the last validator line before
unconditional float-roundtrip parsing. Complete bundled GeoParquet 1.1 and
PROJJSON v0.7 schema validation remains enabled. An explicit rejecting
retriever replaces the newer offline convenience API and prevents unbundled
HTTP/file retrieval. A new unit test exercises both reference schemes.
The package remains unpublished/untagged 0.1.1; the Git revision identifies
this dependency correction. Public APIs, schema documents, geometry identities,
and writer version are unchanged.

Validation on Beast with Rust 1.97.1:

| Command | Result |
| --- | --- |
| `cargo check --no-default-features` | Passed |
| `cargo check --features geotiff` | Passed |
| `cargo check --features geoparquet` | Passed |
| `cargo check --all-features` | Passed |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features` | Passed: 52 tests, no failures or ignored tests |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` | Passed |
| `cargo tree --all-features` | Passed; no GDAL, C PROJ, GEOS, database, GUI or Rerun runtime |
| `git diff --check` | Passed |
| `cargo +1.92.0 check --all-features` | Passed |
| `cargo tree --all-features -e features -i serde_json` | Only default/std; no float_roundtrip or arbitrary_precision |
| `cargo run --all-features --example generate_interoperability_fixtures -- /tmp/vectorizer-shared-crates-255/spatial-regenerated` | Passed |

All five freshly generated Parquet fixtures and `manifest.json` matched the
committed 0.1.1 fixture files byte-for-byte. This proves those fixture bytes,
including pixel/local/EPSG linework and polygon/multipolygon output; it is not
a universal claim for sensitive arbitrary CRS decimal literals. The consumer
upgrade separately compares complete derived bundles against its previous pins.
An independent source review approved the dependency/retriever boundary.
