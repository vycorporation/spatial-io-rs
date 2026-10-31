//! Minimal dependency-light consumer; run without format features.
use spatial_io::{CubicBezier, Point2, UniformSampleOptions, sample_cubic};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let p = [(0., 0.), (1., 2.), (3., 2.), (4., 0.)]
        .map(|(x, y)| Point2::new(x, y))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let curve = CubicBezier::new(p[0], p[1], p[2], p[3]);
    let line = sample_cubic(&curve, UniformSampleOptions::new(60, 61)?)?;
    assert_eq!(line.points().len(), 61);
    assert_eq!(line.points()[0], curve.p0);
    assert_eq!(line.points()[60], curve.p3);
    Ok(())
}
