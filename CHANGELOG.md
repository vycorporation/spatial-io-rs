# Changelog

## 0.1.1 — 2026-10-04

- Pin the offline schema validator to 0.47.0 and explicitly reject unbundled
  references so `geoparquet` does not change consumers' global JSON float
  parsing. Bundled schema validation and the 0.1.1 writer contract are retained
  ([issue #21](https://github.com/vycorporation/spatial-io-rs/issues/21)).

Fixes the five correctness findings from the Sol 6.1 High review of
`f9fd833` ([issue #19](https://github.com/vycorporation/spatial-io-rs/issues/19)).

- Cubic flattening uses exact dyadic De Casteljau subdivision and exact distance
  checks against rounded output chords. It returns `ApproximationPrecision`
  when output endpoint rounding exceeds the tolerance. The conversion profile
  is now `recursive_convex_hull_bound_v2`; coordinate order and source identity
  remain deterministic.
- GeoTIFF adapter `geotiff_reader_0_7_reference_v3` selects the CRS key from
  the declared geographic or projected model. It rejects reserved,
  user-defined, private, missing, geocentric, and unknown-model identities
  rather than substituting an EPSG code. PixelIsPoint normalization and all
  six affine coefficients are retained.
- GeoParquet validates complete CRS structure and GeoParquet 1.1 metadata
  against bundled official schemas, with no network or filesystem schema
  retrieval. Invalid PROJJSON is rejected before publication.
- GeoParquet includes `spatial_io_spatial_reference_v1` metadata containing
  the full spatial reference and writer version. Pixel/local output still
  declares `crs: null`. Supplied affine provenance is revalidated.
- SHA-256 attestation reads the staged file handle before atomic publication,
  so concurrent overwrites cannot mix another artifact's checksum into a
  report. The report identifies this call's artifact even if a later call
  replaces the destination.

The deterministic GeoParquet fixture bytes and hashes change because the
writer version and spatial provenance are now embedded. GeoParquet remains
version 1.1.0. Exact cubic certification uses more arithmetic than profile v1;
the maximum subdivision depth remains 32 and no reprojection is introduced.
An optimized batch benchmark is retained under `benches/flatten_batch.rs`;
see the validation record for measured certification cost.
