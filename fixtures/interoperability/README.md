# Interoperability fixture matrix

This directory contains a deliberately small, synthetic, redistributable
GeoParquet 1.1 matrix. It covers the three coordinate-space cases emitted by
the current `spatial-io` writer:

| File | Coordinate meaning | CRS metadata | Features |
| --- | --- | --- | ---: |
| `pixel.parquet` | top-left, y-down, corner-anchored pixels | explicit `null` | 2 |
| `local.parquet` | local millimetres | explicit `null` | 2 |
| `epsg-32618.parquet` | georeferenced UTM coordinates | EPSG:32618 PROJJSON | 2 |

Every file contains ordered WKB `LineString` geometry, stable source and group
identities, conversion provenance, non-null scalar attributes, and one nullable
floating-point attribute. `manifest.json` pins byte lengths, SHA-256 digests,
row counts, extents, coordinate-space meaning, and expected CRS.

The fixtures are generated entirely from literal synthetic coordinates in
`examples/generate_interoperability_fixtures.rs`. They contain no source image,
third-party dataset, or proprietary style asset. They are licensed under
MIT OR Apache-2.0, matching the repository.

Regenerate the matrix from the repository root:

```bash
cargo run --example generate_interoperability_fixtures \
  --features geoparquet -- fixtures/interoperability
cargo test --all-features --test interoperability_fixtures
git diff -- fixtures/interoperability
```

The contract test independently parses GeoParquet metadata, Arrow schema, and
WKB geometry and verifies the manifest attestations. External validation
commands and observed tool behavior are recorded in
`docs/validation/2026-07-25-interoperability-fixture-matrix.md`.
