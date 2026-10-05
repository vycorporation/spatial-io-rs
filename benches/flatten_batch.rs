use spatial_io::{CubicBezier, FlattenOptions, Point2, flatten_cubic};
use std::hint::black_box;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cubic = CubicBezier::new(
        Point2::new(1.0, 0.0)?,
        Point2::new(1.0, 0.552_284_749_830_793_6)?,
        Point2::new(0.552_284_749_830_793_6, 1.0)?,
        Point2::new(0.0, 1.0)?,
    );
    let options = FlattenOptions::new(0.001)?;
    let sample = flatten_cubic(&cubic, options)?;
    let before = Instant::now();
    for _ in 0..100 {
        black_box(flatten_cubic(black_box(&cubic), options)?);
    }
    println!(
        "100 quarter-circle cubics, tolerance {}, {} vertices each: {:?}",
        options.tolerance(),
        sample.line.points().len(),
        before.elapsed(),
    );
    Ok(())
}
