# Explicit attribute schemas

`FeatureCollectionV1` carries an ordered `attribute_schema` independently of
the row values in each `FeatureV1`. This is required for faithful interchange:
an all-null column still has a real logical type and nullability that cannot be
inferred from its values.

Each `AttributeFieldV1` declares:

- a non-empty unique name that is not a reserved GeoParquet column;
- one `AttributeType` (`Bool`, `I64`, `U64`, `F64`, `Bytes`, or `String`); and
- whether null or absent values are allowed.

The GeoParquet writer emits attribute columns in declaration order. It rejects
duplicate or reserved declarations, undeclared feature attributes, non-null
values of the wrong type, and null or absent values in non-nullable fields.
It emits a declared nullable column even when every feature value is null.

For example, the `vectorizer-rs` fixed fitter intentionally has no deviation
guarantee, so every value in its nullable source column is null:

```rust
use spatial_io::{AttributeFieldV1, AttributeType};

let source_schema = vec![AttributeFieldV1 {
    name: "maximum_deviation_working_pixels".to_owned(),
    value_type: AttributeType::F64,
    nullable: true,
}];
```

The producer then includes `AttributeValue::Null` (or omits the value) on each
feature. The resulting Arrow field remains nullable `Float64`; the writer does
not drop it or guess another type.

## Polygon conversion provenance

Polygon topology is explicit rather than inferred. A producer that creates a
`Polygon` or `MultiPolygon` should use the reserved provenance fields on
`FeatureV1` to retain that decision:

- `source_primitive_id` identifies the source curve, path, mask, or other
  primitive;
- `group_id` records the producer's shell/hole or multipart grouping identity;
- `conversion_profile_id` names the polygonization or literal-input contract;
  and
- `conversion_tolerance` records a meaningful positive tolerance when the
  producer used one.

`write_geoparquet` copies these values unchanged for `LineString`, `Polygon`,
and `MultiPolygon` rows. The library validates topology but does not supply a
polygonizer, infer ring roles from winding, or invent conversion provenance.
See [`polygon-topology.md`](polygon-topology.md) for the complete contract.
