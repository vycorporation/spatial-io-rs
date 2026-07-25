#![allow(clippy::float_cmp)]

use spatial_io::{
    CubicBezier, CubicPath, FlattenOptions, Point2, flatten_cubic, flatten_cubic_path,
};

fn point(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).expect("finite fixture")
}

#[test]
fn straight_cubic_collapses_to_endpoints() -> Result<(), Box<dyn std::error::Error>> {
    let straight = CubicBezier::new(
        point(0.0, 0.0),
        point(1.0, 0.0),
        point(2.0, 0.0),
        point(3.0, 0.0),
    );
    let flattened = flatten_cubic(&straight, FlattenOptions::new(0.01)?)?;
    assert_eq!(flattened.line.points(), &[point(0.0, 0.0), point(3.0, 0.0)]);
    assert_eq!(flattened.profile_id, "recursive_convex_hull_bound_v1");
    Ok(())
}

#[test]
fn tolerance_collapsed_closed_cubic_preserves_a_valid_zero_length_line()
-> Result<(), Box<dyn std::error::Error>> {
    let anchor = point(10.0, 20.0);
    let closed = CubicBezier::new(anchor, point(10.25, 20.0), point(10.0, 20.25), anchor);

    let flattened = flatten_cubic(&closed, FlattenOptions::new(0.5)?)?;

    assert_eq!(flattened.line.points(), &[anchor, anchor]);
    assert_eq!(flattened.subdivision_count, 0);
    Ok(())
}

#[test]
fn tolerance_collapsed_closed_path_preserves_a_valid_zero_length_line()
-> Result<(), Box<dyn std::error::Error>> {
    let anchor = point(10.0, 20.0);
    let path = CubicPath::new(vec![CubicBezier::new(
        anchor,
        point(10.25, 20.0),
        point(10.0, 20.25),
        anchor,
    )])?;

    let flattened =
        flatten_cubic_path(&path, vec!["closed".to_owned()], FlattenOptions::new(0.5)?)?;

    assert_eq!(flattened.line.points(), &[anchor, anchor]);
    assert_eq!(flattened.subdivision_count, 0);
    Ok(())
}

#[test]
fn sampled_curve_stays_within_requested_directed_bound() -> Result<(), Box<dyn std::error::Error>> {
    let cubic = CubicBezier::new(
        point(1.0, 0.0),
        point(1.0, 0.552_284_749_830_793_6),
        point(0.552_284_749_830_793_6, 1.0),
        point(0.0, 1.0),
    );
    let tolerance = 0.001;
    let line = flatten_cubic(&cubic, FlattenOptions::new(tolerance)?)?.line;
    for index in 0..=4096 {
        let t = f64::from(index) / 4096.0;
        let sample = evaluate(&cubic, t);
        let distance = line
            .points()
            .windows(2)
            .map(|segment| distance_to_segment(sample, segment[0], segment[1]))
            .fold(f64::INFINITY, f64::min);
        assert!(distance <= tolerance + 1e-12, "{distance} at t={t}");
    }
    Ok(())
}

#[test]
fn connected_path_has_one_copy_of_each_seam() -> Result<(), Box<dyn std::error::Error>> {
    let first = CubicBezier::new(
        point(0.0, 0.0),
        point(1.0, 0.0),
        point(2.0, 0.0),
        point(3.0, 0.0),
    );
    let second = CubicBezier::new(
        point(3.0, 0.0),
        point(4.0, 0.0),
        point(5.0, 0.0),
        point(6.0, 0.0),
    );
    let path = CubicPath::new(vec![first, second])?;
    let derived = flatten_cubic_path(
        &path,
        vec!["first".to_owned(), "second".to_owned()],
        FlattenOptions::new(0.01)?,
    )?;
    assert_eq!(
        derived.line.points(),
        &[point(0.0, 0.0), point(3.0, 0.0), point(6.0, 0.0)]
    );
    assert_eq!(derived.source_primitive_ids, ["first", "second"]);
    Ok(())
}

#[test]
fn rejects_invalid_tolerance_and_disconnected_paths() {
    assert!(FlattenOptions::new(0.0).is_err());
    assert!(FlattenOptions::new(f64::NAN).is_err());
    let disconnected = CubicPath::new(vec![
        CubicBezier::new(
            point(0.0, 0.0),
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(1.0, 0.0),
        ),
        CubicBezier::new(
            point(2.0, 0.0),
            point(2.0, 0.0),
            point(3.0, 0.0),
            point(3.0, 0.0),
        ),
    ]);
    assert!(disconnected.is_err());
}

fn evaluate(cubic: &CubicBezier, t: f64) -> Point2 {
    let one_minus = 1.0 - t;
    let b0 = one_minus.powi(3);
    let b1 = 3.0 * one_minus.powi(2) * t;
    let b2 = 3.0 * one_minus * t.powi(2);
    let b3 = t.powi(3);
    point(
        cubic.p0.x() * b0 + cubic.p1.x() * b1 + cubic.p2.x() * b2 + cubic.p3.x() * b3,
        cubic.p0.y() * b0 + cubic.p1.y() * b1 + cubic.p2.y() * b2 + cubic.p3.y() * b3,
    )
}

fn distance_to_segment(point: Point2, start: Point2, end: Point2) -> f64 {
    let dx = end.x() - start.x();
    let dy = end.y() - start.y();
    let length_squared = dx.mul_add(dx, dy * dy);
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (((point.x() - start.x()) * dx + (point.y() - start.y()) * dy) / length_squared)
            .clamp(0.0, 1.0)
    };
    (point.x() - dx.mul_add(t, start.x())).hypot(point.y() - dy.mul_add(t, start.y()))
}
