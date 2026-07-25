# spatial-io Context

## Repository role

`spatial-io` owns reusable spatial conversion and format boundaries.
It accepts crate-owned geometry, attributes, coordinate meaning, CRS, and
provenance and returns validated derived geometry or a completed artifact.

It is write-first but not export-named because narrowly scoped readers may be
added when consumers need them.
The first reader obtains spatial reference metadata from a local GeoTIFF; it
does not make raster pixels part of the geometry API.

## Vocabulary

- **Source primitive**: exact producer-owned geometry, such as a cubic Bézier.
- **Derived linework**: portable vertices created through a named conversion
  profile and effective tolerance.
- **Pixel anchor**: whether integer geometry coordinates denote grid corners
  or pixel centers.
- **Raster interpretation**: GeoTIFF `PixelIsArea` or `PixelIsPoint` metadata.
- **Corner-normalized affine**: exact six-coefficient transform whose input
  grid origin denotes the outer raster corner.
- **Local coordinates**: Cartesian coordinates with a declared unit and no CRS.
- **Georeferenced coordinates**: world coordinates with EPSG or PROJJSON CRS.
- **Unknown CRS**: an explicit absence, serialized as GeoParquet `crs: null`.

## Ownership boundaries

`vectorizer-rs` owns raster decoding and compute, contour tracing, cubic
fitting, geometry audits, canonical curve rows, manifests, and previews.
Its future adapter may flatten audited cubics and use source GeoTIFF metadata
through this crate without changing those artifacts.

Rerun owns graph and chunk storage, project intent, UI commands, viewer
selection, interaction, and GPU rendering.
Its future adapter may invoke the same writers for graph-owned geometry.

`attribute-styling-rs` owns classifications, filters, ramps, and visual style
resolution.
`spatial-io` preserves typed attributes but does not render or style them.

## Invariants

- Public APIs contain no Arrow, Parquet, GeoTIFF-reader, vectorizer, Rerun,
  Whitebox, GDAL, PROJ, GEOS, or database types.
- Cubic control points are never mislabeled as LineString vertices.
- Conversion is deterministic and fails closed at resource limits.
- CRS is explicit and never inferred from a filename or dimensions.
- Full affine rotation and skew are retained.
- A closed path is not automatically a polygon.
- Writers stage complete output and publish atomically.
- Default Cargo features remain dependency-light.

## Deferred work

Polygon topology, reprojection, raster warping, remote COG range access,
automatic raster-band interpretation, GeoParquet reading, GeoJSON, broader
QGIS/DuckDB/Sedona validation, and consumer runtime adapters require separate
issues and evidence.
