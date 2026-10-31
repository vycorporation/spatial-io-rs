#![allow(clippy::float_cmp)]
use spatial_io::{CubicBezier, Point2, SpatialIoError, UniformSampleOptions, sample_cubic};
fn cubic(points: [[f64; 2]; 4]) -> CubicBezier {
    let p = points.map(|p| Point2::new(p[0], p[1]).unwrap());
    CubicBezier::new(p[0], p[1], p[2], p[3])
}
#[test]
fn intervals_include_exact_endpoints_and_preserve_closed_degenerate_curves() {
    for points in [
        [[0., 0.], [1., 2.], [3., -2.], [4., 0.]],
        [[2., 2.]; 4],
        [[0., 0.], [2., 3.], [-2., 3.], [0., 0.]],
    ] {
        let c = cubic(points);
        let line = sample_cubic(&c, UniformSampleOptions::new(60, 61).unwrap()).unwrap();
        assert_eq!(line.points().len(), 61);
        assert_eq!(line.points()[0], c.p0);
        assert_eq!(line.points()[60], c.p3);
        let p = line.points()[30];
        assert_eq!(
            p.x(),
            (points[0][0] + 3. * points[1][0] + 3. * points[2][0] + points[3][0]) / 8.
        );
        assert_eq!(
            p.y(),
            (points[0][1] + 3. * points[1][1] + 3. * points[2][1] + points[3][1]) / 8.
        );
    }
}
#[test]
fn invalid_intervals_and_resource_requests_fail_before_allocation() {
    assert!(UniformSampleOptions::new(0, 61).is_err());
    assert!(matches!(
        UniformSampleOptions::new(60, 60),
        Err(SpatialIoError::ResourceLimit { .. })
    ));
    assert!(UniformSampleOptions::new(u32::MAX, 1024).is_err());
}

#[test]
fn one_interval_copies_signed_zero_endpoint_bits() {
    let c = cubic([[-0.0, 0.0], [1., 2.], [3., 2.], [0., -0.0]]);
    let line = sample_cubic(&c, UniformSampleOptions::new(1, 2).unwrap()).unwrap();
    assert_eq!(line.points().len(), 2);
    assert_eq!(line.points()[0].x().to_bits(), (-0.0_f64).to_bits());
    assert_eq!(line.points()[1].y().to_bits(), (-0.0_f64).to_bits());
}
