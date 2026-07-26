# Interoperability fixture matrix validation — 2026-07-25

## Scope

The checked-in matrix under `fixtures/interoperability/` is the stable,
redistributable validation input for the current GeoParquet 1.1 WKB
`LineString` contract. The three synthetic artifacts cover pixel, local, and
EPSG:32618 georeferenced coordinate spaces.

The fixtures are test data, not a runtime dependency. QGIS, DuckDB Spatial,
and SedonaDB remain external validators and are not linked into `spatial-io`.

## Reproduction

```bash
cargo run --example generate_interoperability_fixtures \
  --features geoparquet -- fixtures/interoperability
cargo test --all-features --test interoperability_fixtures
```

The fixture contract test verifies the committed SHA-256 digests and byte
lengths, GeoParquet 1.1 metadata, WKB `LineString` geometry, typed attributes,
feature counts, extents, and explicit CRS/null-CRS behavior.

## Independent readers

The matrix was regenerated and read independently on macOS 15.5 with:

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

QGIS reported two `LineString` features and nine non-geometry fields in every
file. Observed extents and coordinate systems were:

| File | Extent `(xmin, ymin, xmax, ymax)` | CRS |
| --- | --- | --- |
| `pixel.parquet` | `(0, 0, 20, 14)` | none |
| `local.parquet` | `(-2.5, -3, 5.5, 4.5)` | none |
| `epsg-32618.parquet` | `(499990, 4399980, 500025, 4400020)` | WGS 84 / UTM zone 18N, EPSG:32618 |

This independently confirms that pixel and local coordinates do not acquire an
invented geographic CRS and that the projected fixture retains its EPSG
identity.

### DuckDB Spatial

```bash
duckdb -csv -c "INSTALL spatial; LOAD spatial;
SELECT filename, count(*) AS feature_count,
       ST_GeometryType(geometry) AS geometry_type,
       ST_Extent_Agg(geometry) AS extent
FROM read_parquet(
  'fixtures/interoperability/*.parquet',
  filename=true
)
GROUP BY filename, geometry_type
ORDER BY filename;"
```

DuckDB returned two `LINESTRING` rows for each file. Its aggregate extent
polygons matched the manifest bounds exactly.

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

All three validated reads materialized two rows, eleven total columns, and a
`geoarrow.wkb` geometry extension. PROJ and GDAL were not required for these
GeoParquet reads. Geometry-only SQL projections are intentionally avoided
because SedonaDB 0.4.0 has an upstream projected-column-index panic already
recorded in the larger real-output validation.

## Limitations

- Pixel origin, y direction, pixel anchoring, and the local unit are retained
  in the matrix manifest because GeoParquet has no standard field for those
  producer semantics. They are not inferred by external GIS readers.
- The matrix proves the current `LineString` contract only. It does not imply
  polygon topology support.
- Reader validation proves interoperability for the versions and observations
  above; QGIS, DuckDB, and SedonaDB remain outside the Rust dependency graph.
