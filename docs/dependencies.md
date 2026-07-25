# Dependency decisions

All public interfaces remain owned by `spatial-io`.
Exact versions below describe the bootstrap and may change only with contract
tests and a reviewed dependency update.

| Dependency | Bootstrap line | Role | Boundary |
| --- | --- | --- | --- |
| `geotiff-reader` / `geotiff-core` | `0.7.0` | Local GeoTIFF, BigTIFF, tiled layout, GeoKeys, full affine, raster interpretation | Optional `geotiff`; copied into crate-owned values |
| Apache Arrow / Parquet | `58` | Typed columns and Parquet publication | Optional `geoparquet`; private |
| `geoparquet` | `0.8` | GeoParquet 1.1 metadata model and independent metadata parsing | Optional `geoparquet`; private |
| `wkb` | `0.9.2` | Little-endian OGC WKB writing and independent fixture decoding | Optional `geoparquet`; private |
| `geo-types` | `0.7.19` | Private adapter into the WKB writer | Optional `geoparquet`; private |
| `epsg-utils` | `0.0.3`, exact | Embedded EPSG-to-PROJJSON lookup | Optional `geoparquet`; isolated behind `Crs` |

The selected versions support Rust 1.89 and require no system GDAL, PROJ, or
GEOS installation.
The young `epsg-utils` dependency is intentionally exact-pinned and exposed
only through crate-owned errors and CRS values.
Caller-provided PROJJSON remains supported so this resolver is replaceable.

`geotiff-reader` is used instead of the current Whitebox high-level raster
adapter for the first reference path because it retains GeoTIFF
`ModelTransformationTag` rotation/skew and raster interpretation.
Whitebox Next Gen remains a useful broader-raster candidate and
interoperability reference.

DuckDB Spatial, SedonaDB, QGIS, and GeoArrow are validation consumers.
Embedding a database engine merely to serialize geometry would enlarge the
runtime without improving the owned conversion contract.

## Upgrade policy

Dependency updates must:

1. preserve the public dependency firewall;
2. pass all feature combinations and strict validation;
3. retain literal GeoTIFF affine and raster-type fixtures;
4. retain independent GeoParquet metadata and WKB reader assertions;
5. keep MSRV at or below the repository declaration; and
6. document any output-affecting change before merge.
