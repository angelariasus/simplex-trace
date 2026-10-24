//! The exact rational number used for every value in the crate.

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use std::str::FromStr;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

/// An arbitrary-precision exact rational number, always kept in lowest terms.
///
/// It supports the usual arithmetic (`+ - * /`, on values and references), comparisons,
/// [`Display`](fmt::Display) (`3/2`, `-4`, never a decimal) and parsing from the same
/// notation.
///
/// ```
/// use simplex_trace::{Rational, ratio};
///
/// let a = ratio(3, 2);
/// let b: Rational = "7/4".parse().unwrap();
/// let c = Rational::from(5);
/// let d = &a + &b * c;
/// assert_eq!(d, ratio(41, 4));
/// assert_eq!(d.to_string(), "41/4");
/// assert_eq!(d.to_i64_parts(), Some((41, 4)));
/// assert_eq!(d.to_f64(), 10.25);
/// ```
///
/// The representation is an implementation detail, so the crate can change it (for example
/// to speed up small numbers) without breaking users. With the `num-interop` feature it
/// converts to and from [`num_rational::BigRational`].
///
/// With the `serde` feature it is a JSON string such as `"3/2"`. JSON numbers are rejected,
/// so nothing is ever rounded.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rational(BigRational);

impl Rational {
    /// The fraction `numer / denom`, reduced. Panics if `denom == 0`.
    pub fn new(numer: i64, denom: i64) -> Self {
        assert!(denom != 0, "denominator is zero");
        Rational(BigRational::new(BigInt::from(numer), BigInt::from(denom)))
    }

    pub fn zero() -> Self {
        Rational(BigRational::zero())
    }

    pub fn one() -> Self {
        Rational(BigRational::one())
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        self.0.is_positive()
    }

    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }

    /// `true` when the denominator is 1.
    pub fn is_integer(&self) -> bool {
        self.0.is_integer()
    }

    pub fn abs(&self) -> Self {
        Rational(self.0.abs())
    }

    /// Numerator and (positive) denominator in lowest terms, if both fit in an `i64`: for
    /// example to draw a stacked fraction.
    pub fn to_i64_parts(&self) -> Option<(i64, i64)> {
        Some((self.0.numer().to_i64()?, self.0.denom().to_i64()?))
    }

    /// The nearest `f64`, for display or plotting only; computations should stay exact.
    pub fn to_f64(&self) -> f64 {
        self.0.to_f64().unwrap_or(f64::NAN)
    }
}

impl Default for Rational {
    fn default() -> Self {
        Rational::zero()
    }
}

impl From<i32> for Rational {
    fn from(n: i32) -> Self {
        Rational(BigRational::from_integer(BigInt::from(n)))
    }
}

impl From<i64> for Rational {
    fn from(n: i64) -> Self {
        Rational(BigRational::from_integer(BigInt::from(n)))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// Same as [`Display`](fmt::Display): `3/2`.
impl fmt::Debug for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// The error of parsing a [`Rational`] from a string that is not `n` or `n/d` with integers
/// `n` and `d != 0`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseRationalError {
    reason: String,
}

impl fmt::Display for ParseRationalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::error::Error for ParseRationalError {}

/// Parses `n` or `n/d` (integers, `d != 0`), such as `"3"`, `"-1/2"`. Decimals such as
/// `"0.5"` are rejected: write `"1/2"`.
impl FromStr for Rational {
    type Err = ParseRationalError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<BigRational>()
            .map(Rational)
            .map_err(|e| ParseRationalError {
                reason: e.to_string(),
            })
    }
}

macro_rules! binary_op {
    ($trait:ident, $method:ident, $assign_trait:ident, $assign_method:ident) => {
        impl $trait<Rational> for Rational {
            type Output = Rational;
            fn $method(self, rhs: Rational) -> Rational {
                Rational(self.0.$method(rhs.0))
            }
        }
        impl $trait<&Rational> for Rational {
            type Output = Rational;
            fn $method(self, rhs: &Rational) -> Rational {
                Rational(self.0.$method(&rhs.0))
            }
        }
        impl $trait<Rational> for &Rational {
            type Output = Rational;
            fn $method(self, rhs: Rational) -> Rational {
                Rational((&self.0).$method(rhs.0))
            }
        }
        impl $trait<&Rational> for &Rational {
            type Output = Rational;
            fn $method(self, rhs: &Rational) -> Rational {
                Rational((&self.0).$method(&rhs.0))
            }
        }
        impl $assign_trait<Rational> for Rational {
            fn $assign_method(&mut self, rhs: Rational) {
                self.0.$assign_method(rhs.0);
            }
        }
        impl $assign_trait<&Rational> for Rational {
            fn $assign_method(&mut self, rhs: &Rational) {
                self.0.$assign_method(&rhs.0);
            }
        }
    };
}

binary_op!(Add, add, AddAssign, add_assign);
binary_op!(Sub, sub, SubAssign, sub_assign);
binary_op!(Mul, mul, MulAssign, mul_assign);
// Division by zero panics, as for integers.
binary_op!(Div, div, DivAssign, div_assign);

impl Neg for Rational {
    type Output = Rational;
    fn neg(self) -> Rational {
        Rational(-self.0)
    }
}

impl Neg for &Rational {
    type Output = Rational;
    fn neg(self) -> Rational {
        Rational(-&self.0)
    }
}

impl Sum for Rational {
    fn sum<I: Iterator<Item = Rational>>(iter: I) -> Rational {
        iter.fold(Rational::zero(), |acc, x| acc + x)
    }
}

impl<'a> Sum<&'a Rational> for Rational {
    fn sum<I: Iterator<Item = &'a Rational>>(iter: I) -> Rational {
        iter.fold(Rational::zero(), |acc, x| acc + x)
    }
}

#[cfg(feature = "num-interop")]
impl From<BigRational> for Rational {
    fn from(r: BigRational) -> Self {
        Rational(r)
    }
}

#[cfg(feature = "num-interop")]
impl From<Rational> for BigRational {
    fn from(r: Rational) -> Self {
        r.0
    }
}

#[cfg(feature = "num-interop")]
impl Rational {
    /// The underlying [`num_rational::BigRational`].
    pub fn as_big_rational(&self) -> &BigRational {
        &self.0
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Rational {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Rational {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.trim()
            .parse()
            .map_err(|e| serde::de::Error::custom(format!("invalid rational {s:?}: {e}")))
    }
}
