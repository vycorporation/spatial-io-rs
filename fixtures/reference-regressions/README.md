# GeoTIFF CRS regression fixtures

These two synthetic 2 × 2 byte rasters reproduce CRS identity errors found in
the Sol 6.1 High review of `f9fd833`. They contain no third-party data and use
the repository's MIT OR Apache-2.0 license. Both have the corner affine
`[500000, 2, 0, 4400000, 0, -3]` and PixelIsArea interpretation.

`user-defined-projected.tif` was generated with `GeoTiffBuilder` 0.7.0 and
projected CRS key 32767, base geodetic CRS key 4326, citation
`custom transverse mercator`, projection key 32767, method key 3075 = 1,
linear units key 3076 = 9001, longitude/latitude origins -75/0,
false easting/northing 500000/0, and scale 0.9996. It is a custom projected CRS;
32767 is a GeoTIFF sentinel, not its EPSG identity.

`missing-projected-identity.tif` has a projected model with only geographic
base CRS key 4326. Its projected authority identity is absent. Assigning
EPSG:4326 would relabel metre-valued coordinates as longitude/latitude.

| File | SHA-256 | Expected library error |
| --- | --- | --- |
| `user-defined-projected.tif` | `1f13f8568162d927a61c392eaaed290c634a324606b1bf0797b71365b8406907` | `UnsupportedCrs` |
| `missing-projected-identity.tif` | `8d0e051df3de5fb618daa3b36c49e7e76caa53fa4203a20792d1bc80f63a915f` | `MissingCrs` |

Independent validation on Beast on 2026-10-04:

```bash
gdalinfo -json fixtures/reference-regressions/user-defined-projected.tif
gdalinfo -json fixtures/reference-regressions/missing-projected-identity.tif
```

GDAL returned `PROJCRS["custom transverse mercator", …]` with base EPSG:4326
and no projected authority identifier for the first. It returned an unnamed
engineering CRS with unknown datum for the second. Both full affines matched
the literal values above. The integration test exercises the crate against
these exact files; GDAL remains an external validator, not a dependency.
