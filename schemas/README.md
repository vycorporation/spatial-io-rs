# Offline format schemas

These upstream schema snapshots are embedded only by the optional `geoparquet`
adapter. Validation uses `jsonschema` with default features disabled and an
offline registry; it does not fetch caller-supplied schema URLs.

| Snapshot | Source | SHA-256 |
| --- | --- | --- |
| `geoparquet-v1.1.0.schema.json` | <https://geoparquet.org/releases/v1.1.0/schema.json> | `ccc1df2b40aa004383b4dc896e1da6b355a19f2324d832d0605052651f99a054` |
| `projjson-v0.7.schema.json` | <https://proj.org/schemas/v0.7/projjson.schema.json> | `67f090556bae996522edad482597c0f9e1b919ca4c9288ac9f8f7cd4180f49b6` |

GeoParquet is licensed under Apache-2.0; see `GEOPARQUET-LICENSE`.
PROJJSON is copyright Even Rouault and PROJ contributors, 2019–2023, under
the MIT license; see the schema's copyright comment and `PROJ-LICENSE`.

The writer validates PROJJSON against the schema's `crs` definition as well as
validating the complete GeoParquet `geo` metadata. A datum or coordinate
operation alone is not a CRS. Schema validation proves structural conformance;
it does not verify authority database consistency, coordinate epochs, or
reprojection suitability.
