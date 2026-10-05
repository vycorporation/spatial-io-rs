//! Exact predicates on finite binary64 inputs; no coordinate snapping.

use crate::Point2;
use num_rational::BigRational;
use std::cmp::Ordering;

fn rational(value: f64) -> BigRational {
    BigRational::from_float(value).expect("predicate coordinates were validated finite")
}

pub(crate) fn determinant_sign(a: f64, b: f64, c: f64, d: f64) -> Ordering {
    if a == 0.0 || d == 0.0 {
        return if b == 0.0 || c == 0.0 {
            Ordering::Equal
        } else {
            product_sign(b, c).reverse()
        };
    }
    if b == 0.0 || c == 0.0 {
        return product_sign(a, d);
    }
    if let Some(sign) = filtered_difference(a * d, b * c) {
        return sign;
    }
    (rational(a) * rational(d) - rational(b) * rational(c)).cmp(&BigRational::default())
}

pub(crate) struct ExactPoint {
    pub(crate) x: BigRational,
    pub(crate) y: BigRational,
}

impl From<Point2> for ExactPoint {
    fn from(point: Point2) -> Self {
        Self {
            x: rational(point.x()),
            y: rational(point.y()),
        }
    }
}

impl ExactPoint {
    pub(crate) fn midpoint(a: Point2, b: Point2) -> Self {
        let a = Self::from(a);
        let b = Self::from(b);
        let two = BigRational::from_integer(2.into());
        Self {
            x: (a.x + b.x) / &two,
            y: (a.y + b.y) / two,
        }
    }
}

pub(crate) fn exact_orientation(a: &ExactPoint, b: &ExactPoint, c: &ExactPoint) -> Ordering {
    ((&b.x - &a.x) * (&c.y - &a.y) - (&b.y - &a.y) * (&c.x - &a.x)).cmp(&BigRational::default())
}

pub(crate) fn orientation(a: Point2, b: Point2, c: Point2) -> Ordering {
    if a == c || b == c || a == b {
        return Ordering::Equal;
    }
    let left = (b.x() - a.x()) * (c.y() - a.y());
    let right = (b.y() - a.y()) * (c.x() - a.x());
    if let Some(sign) = filtered_difference(left, right) {
        return sign;
    }
    exact_orientation(&a.into(), &b.into(), &c.into())
}

fn product_sign(a: f64, b: f64) -> Ordering {
    if a.is_sign_negative() == b.is_sign_negative() {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

// A deliberately loose bound covers two coordinate subtractions, products,
// and their difference. Only normal products and a finite permanent qualify;
// underflow, overflow, cancellation and exact zero use rational arithmetic.
fn filtered_difference(left: f64, right: f64) -> Option<Ordering> {
    if !left.is_normal() || !right.is_normal() {
        return None;
    }
    let permanent = left.abs() + right.abs();
    let difference = left - right;
    if permanent.is_finite() && difference.abs() > (8.0 * f64::EPSILON) * permanent {
        difference.partial_cmp(&0.0)
    } else {
        None
    }
}
