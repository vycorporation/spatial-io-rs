# Interoperability fixture matrix validation — 2026-07-25

## Scope

The checked-in matrix under `fixtures/interoperability/` is the stable,
redistributable validation input for the current GeoParquet 1.1 WKB
`LineString`, `Polygon`, and `MultiPolygon` contract. Three synthetic artifacts
cover pixel, local, and EPSG:32618 georeferenced linework. One artifact contains
a literal shell with one hole, and one contains two disjoint polygon parts.

The fixtures are test data, not a runtime dependency. QGIS, DuckDB Spatial,
and SedonaDB remain external validators and are not linked into `spatial-io`.

## Reproduction

```bash
cargo run --example generate_interoperability_fixtures \
  --features geoparquet -- fixtures/interoperability
cargo test --all-features --test interoperability_fixtures
```

The fixture contract test verifies the committed SHA-256 digests and byte
lengths, GeoParquet 1.1 metadata, WKB geometry and polygon ring/component
counts, typed attributes, feature counts, extents, and explicit CRS/null-CRS
behavior.

## Independent readers

The matrix was regenerated twice with byte-identical output and read
independently on macOS Tahoe 26.3.1 with:

- QGIS 4.2.0, GDAL/OGR 3.12.4, PROJ 9.8.1, and GEOS 3.14.1;
- DuckDB 1.5.3 with its Spatial extension; and
- SedonaDB 0.4.0 with PyArrow 21 and `geoarrow-pyarrow`.

### QGIS

```bash
for input in fixtures/interoperability/*.parquet; do
  /Applications/QGIS-final-4_2_0.app/Contents/MacOS/qgis_process \
    run gdal:ogrinfojson -- INPUT="$input" \
    ALL_LAYERS=true FEATURES=false \
    OUTPUT="/tmp/$(basename "$input" .parquet)-qgis.json"
done
```

QGIS reported the declared geometry type, feature count, and nine non-geometry
fields in every file. Observed values were:

| File | Type | Rows | Extent `(xmin, ymin, xmax, ymax)` | CRS |
| --- | --- | ---: | --- | --- |
| `pixel.parquet` | `LineString` | 2 | `(0, 0, 20, 14)` | none |
| `local.parquet` | `LineString` | 2 | `(-2.5, -3, 5.5, 4.5)` | none |
| `epsg-32618.parquet` | `LineString` | 2 | `(499990, 4399980, 500025, 4400020)` | WGS 84 / UTM zone 18N, EPSG:32618 |
| `polygon.parquet` | `Polygon` | 1 | `(0, 0, 8, 6)` | none |
| `multipolygon.parquet` | `MultiPolygon` | 1 | `(10, 0, 18, 5)` | none |

This independently confirms that pixel and local coordinates do not acquire an
invented geographic CRS, the projected fixture retains its EPSG identity, and
polygonal WKB is identified without coercion.

### DuckDB Spatial

```bash
duckdb -csv -c "INSTALL spatial; LOAD spatial;
SELECT filename, count(*) AS feature_count,
       ST_GeometryType(geometry) AS geometry_type,
       bool_and(ST_IsValid(geometry)) AS all_valid,
       ST_Extent_Agg(geometry) AS extent
FROM read_parquet(
  'fixtures/interoperability/*.parquet',
  filename=true
)
GROUP BY filename, geometry_type
ORDER BY filename;"
```

DuckDB returned the expected `LINESTRING`, `POLYGON`, and `MULTIPOLYGON` types
and row counts. `ST_IsValid` returned true for every row, including the
shell-with-hole and multipart fixtures, and aggregate extent polygons matched
the manifest bounds exactly.

### SedonaDB

```bash
uv run --no-project \
  --with 'sedonadb==0.4.0' \
  --with 'pyarrow==21.0.0' \
  --with geoarrow-pyarrow \
  python - <<'PY'
from pathlib import Path
import sedonadb

sd = sedonadb.connect()
for path in sorted(Path("fixtures/interoperability").glob("*.parquet")):
    table = sd.read_parquet(path, validate=True).to_arrow_table()
    print(path.name, table.num_rows, len(table.schema), table.schema.field("geometry").type)
PY
```

All five validated reads materialized eleven total columns and a
`geoarrow.wkb` geometry extension. The three line files materialized two rows;
each polygonal file materialized one. PROJ and GDAL were not required for
these GeoParquet reads. Geometry-only SQL projections are intentionally
avoided because SedonaDB 0.4.0 has an upstream projected-column-index panic
already recorded in the larger real-output validation.

## Limitations

- Pixel origin, y direction, pixel anchoring, and the local unit are retained
  in the matrix manifest because GeoParquet has no standard field for those
  producer semantics. They are not inferred by external GIS readers.
- The polygon fixtures prove the literal validated topology represented here;
  they do not provide a polygonizer or auto-promote arbitrary closed paths.
- Reader validation proves interoperability for the versions and observations
  above; QGIS, DuckDB, and SedonaDB remain outside the Rust dependency graph.
