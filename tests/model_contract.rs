#![allow(clippy::float_cmp)]

use spatial_io::{Affine2D, Crs, PixelAnchor, Point2};

#[test]
fn rejects_invalid_points_and_projjson() {
    assert!(Point2::new(f64::NAN, 0.0).is_err());
    assert!(Crs::projjson("[]").is_err());
    assert!(Crs::epsg(0).is_err());
}

#[test]
fn applies_all_six_affine_coefficients() -> Result<(), Box<dyn std::error::Error>> {
    let affine = Affine2D::new(100.0, 2.0, 0.5, 200.0, -0.25, -3.0)?;
    assert_eq!(
        affine.transform(Point2::new(4.0, 5.0)?, PixelAnchor::Corner)?,
        Point2::new(110.5, 184.0)?
    );
    assert_eq!(
        affine.transform(Point2::new(4.0, 5.0)?, PixelAnchor::Center)?,
        Point2::new(111.75, 182.375)?
    );
    Ok(())
}

#[test]
fn rejects_singular_affines() {
    assert!(Affine2D::new(0.0, 1.0, 2.0, 0.0, 2.0, 4.0).is_err());
}

#[test]
fn deserialization_preserves_geometry_invariants() {
    use spatial_io::{CubicPath, LineString};
    assert!(serde_json::from_str::<CubicPath>(r#"{"segments":[]}"#).is_err());
    for invalid in [r#"{"points":[]}"#, r#"{"points":[{"x":0,"y":0}]}"#] {
        assert!(serde_json::from_str::<LineString>(invalid).is_err());
    }
    let segment = serde_json::json!({"p0":{"x":0,"y":0},"p1":{"x":1,"y":0},"p2":{"x":2,"y":0},"p3":{"x":3,"y":0}});
    assert!(
        serde_json::from_value::<CubicPath>(
            serde_json::json!({"segments":[segment.clone(),segment]})
        )
        .is_err()
    );
    let line = LineString::new(vec![
        Point2::new(0., 0.).unwrap(),
        Point2::new(1., 1.).unwrap(),
    ])
    .unwrap();
    assert_eq!(
        serde_json::from_value::<LineString>(serde_json::to_value(&line).unwrap()).unwrap(),
        line
    );
}

#[test]
fn serde_rejects_nonfinite_points_and_singular_affines() {
    use serde::Deserialize;
    use serde::de::value::{Error, MapDeserializer};
    let input = MapDeserializer::<_, Error>::new([("x", f64::NAN), ("y", 0.)].into_iter());
    assert!(Point2::deserialize(input).is_err());
    let input = serde_json::json!({"origin_x":0,"x_scale":1,"x_skew":2,"origin_y":0,"y_skew":2,"y_scale":4});
    assert!(serde_json::from_value::<Affine2D>(input).is_err());
}

#[test]
fn affine_invertibility_is_independent_of_units() {
    for scale in [1e-200, 1e-9, 1., 1e200] {
        let affine = Affine2D::new(0., scale, 0., 0., 0., -scale).unwrap();
        assert_eq!(
            affine
                .transform(Point2::new(1., 1.).unwrap(), PixelAnchor::Corner)
                .unwrap(),
            Point2::new(scale, -scale).unwrap()
        );
        assert!(Affine2D::new(0., scale, 2. * scale, 0., 2. * scale, 4. * scale).is_err());
    }
    // Exact cancellation must not be confused with one-sided FMA rounding.
    assert!(Affine2D::new(0., 1.1, 1.1, 0., 1.1, 1.1).is_err());
    assert!(Affine2D::new(0., 1., 1., 0., 1., f64::from_bits(1f64.to_bits() + 1)).is_ok());
}

#[test]
fn mutated_affines_fail_before_transforming() {
    let mut affine = Affine2D::new(0., 1., 0., 0., 0., 1.).unwrap();
    affine.y_scale = 0.;
    assert!(
        affine
            .transform(Point2::new(1., 1.).unwrap(), PixelAnchor::Corner)
            .is_err()
    );
    affine.y_scale = f64::INFINITY;
    assert!(
        affine
            .transform(Point2::new(0., 0.).unwrap(), PixelAnchor::Corner)
            .is_err()
    );
    let overflow = Affine2D::new(f64::MAX, 1., 0., 0., 0., 1.).unwrap();
    assert!(
        overflow
            .transform(Point2::new(f64::MAX, 0.).unwrap(), PixelAnchor::Corner)
            .is_err()
    );
}
