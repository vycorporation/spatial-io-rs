# spatial-io

`spatial-io` is a reusable Rust library for turning typed spatial primitives
into portable linework and topology-validated polygonal artifacts.
It supports pixel, local, and georeferenced coordinates without assuming that
all spatial data is geographic.

The first release provides:

- deterministic, tolerance-bounded cubic Bézier to `LineString` conversion;
- exact six-coefficient affine transformation with explicit pixel anchoring;
- optional local GeoTIFF spatial-reference reading;
- explicit, validated linear-ring, polygon, and multipolygon contracts;
- optional GeoParquet 1.1 WKB `LineString`, `Polygon`, and `MultiPolygon`
  writing;
- explicitly typed scalar-attribute schemas, including all-null columns, and
  conversion provenance; and
- atomic, attested artifact publication.

The Cargo package is `spatial-io`; Rust code imports it as `spatial_io`.
Default features are empty.

## Why a separate crate?

Spatial conversion is needed by
[`vectorizer-rs`](https://github.com/vycorporation/vectorizer-rs), the
[`vycorporation/rerun`](https://github.com/vycorporation/rerun) graph
workbench, and future headless tools.
This repository keeps that contract independent of raster vectorization,
rendering, graph UI, and database runtimes.

`vectorizer-rs` remains authoritative for canonical cubic output and
`preview.png`.
Rerun remains authoritative for graph, UI, and rendering behavior.
Consumers adapt their native records into crate-owned `spatial-io` types.

## Example

```rust
use spatial_io::{
    Affine2D, CubicBezier, CubicPath, FlattenOptions, PixelAnchor, Point2,
    flatten_cubic_path, transform_line_string,
};

let cubic = CubicBezier::new(
    Point2::new(0.0, 0.0)?,
    Point2::new(20.0, 0.0)?,
    Point2::new(20.0, 20.0)?,
    Point2::new(40.0, 20.0)?,
);
let path = CubicPath::new(vec![cubic])?;
let derived = flatten_cubic_path(
    &path,
    vec!["curve-0".to_owned()],
    FlattenOptions::new(0.25)?,
)?;

let pixel_to_world = Affine2D::new(500_000.0, 0.1, 0.0, 4_400_000.0, 0.0, -0.1)?;
let world_line = transform_line_string(
    &derived.line,
    pixel_to_world,
    PixelAnchor::Corner,
)?;
# Ok::<(), spatial_io::SpatialIoError>(())
```

Enable formats only where needed:

```toml
[dependencies]
spatial-io = { git = "https://github.com/vycorporation/spatial-io-rs", features = ["geotiff", "geoparquet"] }
```

See [`examples/cubic_to_geoparquet.rs`](examples/cubic_to_geoparquet.rs) for
complete local and georeferenced output construction.
See [`docs/attributes.md`](docs/attributes.md) for the exact declared-schema
contract used by integrations such as `vectorizer-rs`.
See [`docs/polygon-topology.md`](docs/polygon-topology.md) before constructing
polygon output; ring roles and multipart grouping are always explicit.
See [`fixtures/interoperability/`](fixtures/interoperability/) for the
checked-in pixel, local, and georeferenced GeoParquet validation matrix and
exact independent-reader commands.

## Coordinate and format rules

- Pixel coordinates declare origin, y direction, and whether integers denote
  corners or centers.
- A GeoTIFF reference returns a corner-normalized affine while retaining the
  original `PixelIsArea` or `PixelIsPoint` interpretation.
- Rotation and skew are preserved.
- Unknown, pixel, and local GeoParquet coordinates emit explicit `"crs": null`;
  the writer never accidentally implies OGC:CRS84.
- EPSG identities resolve to PROJJSON for GeoParquet metadata.
- Closed cubic paths and `LineString` values remain linework unless a producer
  explicitly constructs a validated `Polygon` or `MultiPolygon`.
- Rings must be explicitly closed and simple. Shell/hole roles never come from
  winding, and rejected paths are never repaired or auto-promoted.

## Dependency boundary

The core has no GDAL, C PROJ, GEOS, database, GUI, or rendering dependency.
GeoTIFF and GeoParquet support are optional pure-Rust adapters.
Whitebox Next Gen, DuckDB Spatial, SedonaDB, QGIS, and GeoArrow are useful
interoperability consumers, not required runtimes.
See [`docs/dependencies.md`](docs/dependencies.md).

## Validation

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
git diff --check
```

Ordinary development and release validation use the exact Rust 1.97.1
toolchain declared in `rust-toolchain.toml`.
The crate's MSRV is Rust 1.92 so it remains consumable by the current
`vycorporation/rerun` 0.34 workspace.
The public model remains crate-owned; Arrow and Parquet 58 stay private to the
optional GeoParquet adapter.
The crate uses Rust 2024 and forbids unsafe code.

## License

Licensed under either Apache-2.0 or MIT, at your option.
