use spatial_io::{
    LineString, LinearRing, MultiPolygon, Point2, Polygon, RingWinding, SpatialIoError,
};

#[test]
fn ring_preserves_literal_coordinates_and_reports_numeric_winding()
-> Result<(), Box<dyn std::error::Error>> {
    let counterclockwise = points(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)])?;
    let clockwise = counterclockwise.iter().copied().rev().collect::<Vec<_>>();

    let positive_ring = LinearRing::new(counterclockwise.clone())?;
    let negative_ring = LinearRing::new(clockwise.clone())?;

    assert_eq!(positive_ring.points(), counterclockwise);
    assert_eq!(positive_ring.winding(), RingWinding::CounterClockwise);
    assert_eq!(negative_ring.points(), clockwise);
    assert_eq!(negative_ring.winding(), RingWinding::Clockwise);
    Ok(())
}

#[test]
fn ring_rejects_structural_and_topological_invalidity() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        vec![(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
        vec![(0.0, 0.0), (4.0, 0.0), (4.0, 0.0), (0.0, 4.0), (0.0, 0.0)],
        vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (0.0, 0.0)],
        vec![(0.0, 0.0), (4.0, 4.0), (0.0, 5.0), (4.0, 0.0), (0.0, 0.0)],
        vec![
            (0.0, 0.0),
            (4.0, 0.0),
            (4.0, 4.0),
            (2.0, 2.0),
            (0.0, 4.0),
            (2.0, 2.0),
            (0.0, 0.0),
        ],
    ];

    for coordinates in cases {
        assert!(matches!(
            LinearRing::new(points(&coordinates)?),
            Err(SpatialIoError::InvalidGeometry(_))
        ));
    }
    Ok(())
}

#[test]
fn ring_rejects_adjacent_backtracking_edges() -> Result<(), Box<dyn std::error::Error>> {
    let backtracking = points(&[
        (0.0, 0.0),
        (4.0, 0.0),
        (2.0, 0.0),
        (4.0, 4.0),
        (0.0, 4.0),
        (0.0, 0.0),
    ])?;
    assert!(matches!(
        LinearRing::new(backtracking),
        Err(SpatialIoError::InvalidGeometry(_))
    ));
    Ok(())
}

#[test]
fn deserialization_cannot_bypass_ring_validation() {
    let open_ring = serde_json::json!({
        "points": [
            {"x": 0.0, "y": 0.0},
            {"x": 4.0, "y": 0.0},
            {"x": 4.0, "y": 4.0},
            {"x": 0.0, "y": 4.0}
        ],
        "winding": "CounterClockwise"
    });
    let wrong_winding = serde_json::json!({
        "points": [
            {"x": 0.0, "y": 0.0},
            {"x": 4.0, "y": 0.0},
            {"x": 4.0, "y": 4.0},
            {"x": 0.0, "y": 4.0},
            {"x": 0.0, "y": 0.0}
        ],
        "winding": "Clockwise"
    });

    assert!(serde_json::from_value::<LinearRing>(open_ring).is_err());
    assert!(serde_json::from_value::<LinearRing>(wrong_winding).is_err());
}

#[test]
fn closed_linestring_remains_linework_when_polygon_validation_rejects_it()
-> Result<(), Box<dyn std::error::Error>> {
    let bow_tie = points(&[(0.0, 0.0), (4.0, 4.0), (0.0, 5.0), (4.0, 0.0), (0.0, 0.0)])?;
    assert!(LineString::new(bow_tie.clone()).is_ok());
    assert!(matches!(
        LinearRing::new(bow_tie),
        Err(SpatialIoError::InvalidGeometry(_))
    ));
    Ok(())
}

#[test]
fn polygon_uses_explicit_shell_and_hole_roles_without_winding_inference()
-> Result<(), Box<dyn std::error::Error>> {
    let exterior = rectangle(0.0, 0.0, 10.0, 10.0)?;
    let interior = rectangle(2.0, 2.0, 4.0, 4.0)?;
    assert_eq!(exterior.winding(), interior.winding());

    let polygon = Polygon::new(exterior.clone(), vec![interior.clone()])?;
    assert_eq!(polygon.exterior(), &exterior);
    assert_eq!(polygon.interiors(), &[interior]);
    Ok(())
}

#[test]
fn polygon_rejects_holes_outside_touching_overlapping_or_nested()
-> Result<(), Box<dyn std::error::Error>> {
    let exterior = rectangle(0.0, 0.0, 10.0, 10.0)?;
    let invalid_hole_sets = [
        vec![rectangle(12.0, 12.0, 14.0, 14.0)?],
        vec![rectangle(0.0, 2.0, 2.0, 4.0)?],
        vec![
            rectangle(2.0, 2.0, 6.0, 6.0)?,
            rectangle(4.0, 4.0, 8.0, 8.0)?,
        ],
        vec![
            rectangle(2.0, 2.0, 8.0, 8.0)?,
            rectangle(3.0, 3.0, 4.0, 4.0)?,
        ],
    ];
    for holes in invalid_hole_sets {
        assert!(matches!(
            Polygon::new(exterior.clone(), holes),
            Err(SpatialIoError::InvalidGeometry(_))
        ));
    }
    Ok(())
}

#[test]
fn multipolygon_accepts_disjoint_parts_and_isolated_point_contact()
-> Result<(), Box<dyn std::error::Error>> {
    let first = Polygon::new(rectangle(0.0, 0.0, 2.0, 2.0)?, vec![])?;
    let point_touching = Polygon::new(rectangle(2.0, 2.0, 4.0, 4.0)?, vec![])?;
    let disjoint = Polygon::new(rectangle(8.0, 8.0, 9.0, 9.0)?, vec![])?;

    let multipart = MultiPolygon::new(vec![
        first.clone(),
        point_touching.clone(),
        disjoint.clone(),
    ])?;
    assert_eq!(multipart.polygons(), &[first, point_touching, disjoint]);
    Ok(())
}

#[test]
fn multipolygon_rejects_overlap_containment_and_shared_edges()
-> Result<(), Box<dyn std::error::Error>> {
    let first = Polygon::new(rectangle(0.0, 0.0, 4.0, 4.0)?, vec![])?;
    let invalid_seconds = [
        Polygon::new(rectangle(3.0, 3.0, 6.0, 6.0)?, vec![])?,
        Polygon::new(rectangle(1.0, 1.0, 2.0, 2.0)?, vec![])?,
        Polygon::new(rectangle(4.0, 0.0, 6.0, 4.0)?, vec![])?,
    ];
    for second in invalid_seconds {
        assert!(matches!(
            MultiPolygon::new(vec![first.clone(), second]),
            Err(SpatialIoError::InvalidGeometry(_))
        ));
    }
    Ok(())
}

fn points(coordinates: &[(f64, f64)]) -> Result<Vec<Point2>, SpatialIoError> {
    coordinates
        .iter()
        .map(|&(x, y)| Point2::new(x, y))
        .collect()
}

fn rectangle(xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Result<LinearRing, SpatialIoError> {
    LinearRing::new(points(&[
        (xmin, ymin),
        (xmax, ymin),
        (xmax, ymax),
        (xmin, ymax),
        (xmin, ymin),
    ])?)
}

#[test]
fn rejects_inscribed_multipart_interiors_even_when_all_vertices_touch() {
    let square = Polygon::new(rectangle(0., 0., 2., 2.).unwrap(), vec![]).unwrap();
    let diamond = Polygon::new(
        LinearRing::new(points(&[(1., 0.), (2., 1.), (1., 2.), (0., 1.), (1., 0.)]).unwrap())
            .unwrap(),
        vec![],
    )
    .unwrap();
    assert!(MultiPolygon::new(vec![square.clone(), diamond.clone()]).is_err());
    assert!(MultiPolygon::new(vec![diamond, square]).is_err());
}

#[test]
fn winding_is_translation_and_scale_invariant() {
    for (offset, size) in [(0., 1.), (1e10, 1.), (1e14, 1.), (0., 1e-200), (0., 1e200)] {
        let positions = points(&[
            (offset, offset),
            (offset + size, offset),
            (offset + size, offset + size),
            (offset, offset + size),
            (offset, offset),
        ])
        .unwrap();
        let ring = LinearRing::new(positions.clone()).unwrap();
        assert_eq!(ring.winding(), RingWinding::CounterClockwise);
        assert_eq!(
            LinearRing::new(positions.into_iter().rev().collect())
                .unwrap()
                .winding(),
            RingWinding::Clockwise
        );
    }
}

#[test]
fn exact_collinearity_cannot_hide_backtracking() {
    assert!(
        LinearRing::new(points(&[(0., 0.), (1.1, 1.1), (0.55, 0.55), (1., 2.), (0., 0.)]).unwrap())
            .is_err()
    );
}

#[test]
fn inscribed_component_inside_a_hole_remains_disjoint() {
    let container = Polygon::new(
        rectangle(-1., -1., 3., 3.).unwrap(),
        vec![rectangle(0., 0., 2., 2.).unwrap()],
    )
    .unwrap();
    let diamond = Polygon::new(
        LinearRing::new(points(&[(1., 0.), (2., 1.), (1., 2.), (0., 1.), (1., 0.)]).unwrap())
            .unwrap(),
        vec![],
    )
    .unwrap();
    assert!(MultiPolygon::new(vec![container, diamond]).is_ok());
}

#[test]
fn multipart_contacts_are_checked_between_adjacent_binary64_coordinates() {
    let unit = f64::from_bits(1);
    let square = Polygon::new(rectangle(0., 0., 2. * unit, 2. * unit).unwrap(), vec![]).unwrap();
    let diamond = Polygon::new(
        LinearRing::new(
            points(&[
                (unit, 0.),
                (2. * unit, unit),
                (unit, 2. * unit),
                (0., unit),
                (unit, 0.),
            ])
            .unwrap(),
        )
        .unwrap(),
        vec![],
    )
    .unwrap();
    assert!(MultiPolygon::new(vec![square, diamond.clone()]).is_err());
    let holed = Polygon::new(
        rectangle(-unit, -unit, 3. * unit, 3. * unit).unwrap(),
        vec![rectangle(0., 0., 2. * unit, 2. * unit).unwrap()],
    )
    .unwrap();
    assert!(MultiPolygon::new(vec![holed, diamond]).is_ok());
}

#[test]
fn multiple_external_contacts_on_one_edge_remain_valid() {
    let below = Polygon::new(rectangle(0., -2., 4., 0.).unwrap(), vec![]).unwrap();
    let above = Polygon::new(
        LinearRing::new(
            points(&[
                (0., 1.),
                (1., 0.),
                (2., 1.),
                (3., 0.),
                (4., 1.),
                (4., 3.),
                (0., 3.),
                (0., 1.),
            ])
            .unwrap(),
        )
        .unwrap(),
        vec![],
    )
    .unwrap();
    assert!(MultiPolygon::new(vec![below.clone(), above.clone()]).is_ok());
    assert!(MultiPolygon::new(vec![above, below]).is_ok());
}
