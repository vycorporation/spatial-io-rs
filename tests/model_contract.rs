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
