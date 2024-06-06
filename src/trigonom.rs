use core::convert::From;
/// Rational Trigonometry is a new approach to classical trigonometry, developed by Norman
/// Wildberger, that aims to simplify and clarify the subject by using only rational numbers
/// and operations, rather than irrational numbers and limits.
///
/// In traditional trigonometry, concepts such as the sine, cosine, and tangent of an angle
/// are typically defined using circles and the unit circle in particular. These definitions
/// involve irrational numbers and limits, which can make the subject more difficult to
/// understand and work with.
///
/// In rational trigonometry, Wildberger replaces these circular definitions with ones based
/// on lines and line segments, which allows for a more straightforward and intuitive approach.
/// The fundamental concepts in rational trigonometry are the "quadaverage" and the "dilated
/// directed angle," which are defined in terms of lines and line segments, rather than circles.
///
/// Rational trigonometry has been gaining popularity in recent years, as it provides a useful
/// alternative to traditional trigonometry for certain applications, such as computer graphics,
/// robotics, and physics. It can also be a helpful tool for students who struggle with the
/// irrational numbers and limits used in traditional trigonometry.
///
/// In summary, Rational Trigonometry is a new approach to classical trigonometry that uses
/// rational numbers and operations, rather than irrational numbers and limits, making it a more
/// straightforward and intuitive subject to understand and work with.
use core::ops::{Add, Mul, Sub};

/// The function `archimedes` calculates the area of a triangle using Archimedes' formula with the
/// lengths of the three sides provided as `Fraction<i64>` values.
///
/// Arguments:
///
/// * `q_1`: Represents the length of the first side of the triangle.
/// * `q_2`: The parameters `q_1`, `q_2`, and `q_3` represent the lengths of the sides of a triangle. In
///          the context of Archimedes' formula for the area of a triangle, `q_1`, `q_2`, and `q_3`
/// * `q_3`: The parameter `q_3` represents the length of the third side of the triangle.
///
/// Returns:
///
/// The function `archimedes` returns the area of a triangle computed using Archimedes' formula, given
/// the lengths of the 3 sides.
///
/// Example:
///
/// ```rust
/// use num_rational::Rational32;
/// use netlistx_rs::trigonom::archimedes;
/// let q_1 = Rational32::new(1, 2);
/// let q_2 = Rational32::new(1, 4);
/// let q_3 = Rational32::new(1, 6);
/// let quadrea = archimedes(&q_1, &q_2, &q_3);
/// assert_eq!(quadrea, Rational32::new(23, 144));
/// ```
#[inline]
pub fn archimedes<T>(q_1: &T, q_2: &T, q_3: &T) -> T
where
    T: std::marker::Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T> + From<i32>,
{
    let temp = *q_1 + *q_2 - *q_3;
    T::from(4) * *q_1 * *q_2 - temp * temp
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_rational::Ratio;
    // use fractions::Fraction;

    #[test]
    fn test_archimedes2() {
        let q_1: i64 = 1;
        let q_2: i64 = 2;
        let q_3: i64 = 3;
        assert_eq!(archimedes(&q_1, &q_2, &q_3), 8);
    }

    #[test]
    fn test_archimedes3() {
        let q_1 = 1.0;
        let q_2 = 2.0;
        let q_3 = 3.0;
        assert_eq!(archimedes(&q_1, &q_2, &q_3), 8.0);
    }

    #[test]
    fn test_archimedes() {
        let q_1 = Ratio::<i32>::new(1, 2);
        let q_2 = Ratio::<i32>::new(1, 4);
        let q_3 = Ratio::<i32>::new(1, 6);
        assert_eq!(archimedes(&q_1, &q_2, &q_3), Ratio::<i32>::new(23, 144));
    }

    // #[test]
    // fn test_archimedes4() {
    //     let q_1 = Fraction::<i64>::new(1, 2);
    //     let q_2 = Fraction::<i64>::new(1, 4);
    //     let q_3 = Fraction::<i64>::new(1, 6);
    //     assert_eq!(archimedes(&q_1, &q_2, &q_3), Fraction::<i64>::new(23, 144));
    // }
}
